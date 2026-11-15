//! Core runtime lifecycle supervisor, task coordination, and graceful shutdown.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, RwLock};
use std::time::Duration;

use aissistant_storage::SqliteDb;
use serde::{Deserialize, Serialize};

use crate::error::{FfiError, FfiResult};
use crate::events::{BridgeEvent, EventStreamSink};

/// Operating state of the core assistant runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Subsystem allocated but not yet started or configured.
    Uninitialized,
    /// Bootstrapping database connections, migrations, and identity keys.
    Starting,
    /// Fully operational and ready for requests.
    Running,
    /// Running with non-fatal capability degradation or resource limits.
    Degraded,
    /// Two-phase shutdown initiated: rejecting new work and draining active tasks.
    ShuttingDown,
    /// All tasks terminated, database flushed, runtime finalized.
    Terminated,
}

impl LifecycleState {
    /// Returns the static string representation of this state.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Degraded => "degraded",
            Self::ShuttingDown => "shutting_down",
            Self::Terminated => "terminated",
        }
    }

    /// Returns true if the runtime is operational and accepting tasks.
    #[must_use]
    pub const fn is_operational(&self) -> bool {
        matches!(self, Self::Running | Self::Degraded)
    }

    /// Returns true if the runtime is in a terminal state.
    #[must_use]
    pub const fn is_terminal(&self) -> bool {
        matches!(self, Self::Terminated)
    }

    /// Verifies if a transition to `next` is structurally valid.
    #[must_use]
    pub fn can_transition_to(&self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Uninitialized, Self::Starting)
                | (
                    Self::Starting,
                    Self::Running | Self::Degraded | Self::ShuttingDown | Self::Terminated,
                )
                | (Self::Running, Self::Degraded | Self::ShuttingDown)
                | (Self::Degraded, Self::Running | Self::ShuttingDown)
                | (Self::ShuttingDown, Self::Terminated)
                | (Self::Terminated, Self::Starting)
        )
    }
}

/// Reason triggering runtime termination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShutdownReason {
    /// Explicit shutdown invoked by the user or UI close.
    UserRequested,
    /// Operating system signal (SIGINT, SIGTERM, Window close).
    OsSignal,
    /// Operating system low memory eviction.
    MemoryPressure,
    /// Critical unrecoverable database or cryptographic error.
    FatalError,
    /// Automated test harness teardown.
    Testing,
}

impl ShutdownReason {
    /// Returns static string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::UserRequested => "user_requested",
            Self::OsSignal => "os_signal",
            Self::MemoryPressure => "memory_pressure",
            Self::FatalError => "fatal_error",
            Self::Testing => "testing",
        }
    }
}

/// Host OS memory pressure notification level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPressureLevel {
    /// Moderate memory pressure: trim unused memory and caches.
    Moderate,
    /// Critical memory pressure: aggressive cache flush and SQLite checkpointing.
    Critical,
}

impl MemoryPressureLevel {
    /// Returns static string representation.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Moderate => "moderate",
            Self::Critical => "critical",
        }
    }
}

/// Thread-safe cooperative cancellation token for background workers.
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    /// Creates a new uncancelled token.
    #[must_use]
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Returns true if cancellation has been requested.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    /// Requests cancellation of all workers observing this token.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }
}

/// Core runtime lifecycle supervisor managing state transitions and workers.
pub struct CoreRuntime {
    state: RwLock<LifecycleState>,
    shutdown_token: CancellationToken,
    tasks: Mutex<HashMap<u64, String>>,
    task_condvar: Condvar,
    next_task_id: AtomicU64,
    event_sink: Arc<EventStreamSink>,
}

impl std::fmt::Debug for CoreRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoreRuntime")
            .field("state", &self.state.read().ok().map(|s| *s))
            .field("active_tasks", &self.active_task_count())
            .finish()
    }
}

impl CoreRuntime {
    /// Creates a new lifecycle supervisor starting in `Uninitialized` state.
    #[must_use]
    pub fn new(event_sink: Arc<EventStreamSink>) -> Self {
        Self {
            state: RwLock::new(LifecycleState::Uninitialized),
            shutdown_token: CancellationToken::new(),
            tasks: Mutex::new(HashMap::new()),
            task_condvar: Condvar::new(),
            next_task_id: AtomicU64::new(1),
            event_sink,
        }
    }

    /// Returns the current lifecycle state.
    pub fn state(&self) -> LifecycleState {
        self.state
            .read()
            .map(|guard| *guard)
            .unwrap_or(LifecycleState::Terminated)
    }

    /// Transitions the runtime to a new lifecycle state if valid.
    pub fn transition_to(&self, new_state: LifecycleState, reason: Option<&str>) -> FfiResult<()> {
        let mut guard = self
            .state
            .write()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;

        let current = *guard;
        if current == new_state {
            return Ok(());
        }

        if !current.can_transition_to(new_state) {
            return Err(FfiError::InvalidStateTransition {
                from: current.as_str().to_string(),
                to: new_state.as_str().to_string(),
            });
        }

        *guard = new_state;

        // Broadcast state change event
        self.event_sink
            .broadcast(BridgeEvent::LifecycleStateChanged {
                previous: current.as_str().to_string(),
                current: new_state.as_str().to_string(),
                reason: reason.map(ToString::to_string),
            });

        Ok(())
    }

    /// Spawns a tracked background task, returning its unique task ID and cancellation token.
    /// Fails closed if the runtime is shutting down or terminated.
    pub fn spawn_task(&self, name: &str) -> FfiResult<(u64, CancellationToken)> {
        let current_state = self.state();
        if !current_state.is_operational() && current_state != LifecycleState::Starting {
            return Err(FfiError::ShuttingDown);
        }

        let task_id = self.next_task_id.fetch_add(1, Ordering::SeqCst);
        let mut tasks = self
            .tasks
            .lock()
            .map_err(|e| FfiError::LockPoisoned(e.to_string()))?;

        tasks.insert(task_id, name.to_string());
        Ok((task_id, self.shutdown_token.clone()))
    }

    /// Marks a tracked background task as finished and notifies any waiting supervisor.
    pub fn finish_task(&self, task_id: u64) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.remove(&task_id);
            if tasks.is_empty() {
                self.task_condvar.notify_all();
            }
        }
    }

    /// Returns the number of currently active background tasks.
    #[must_use]
    pub fn active_task_count(&self) -> usize {
        self.tasks.lock().map(|t| t.len()).unwrap_or(0)
    }

    /// Returns the shared cancellation token.
    #[must_use]
    pub fn cancellation_token(&self) -> CancellationToken {
        self.shutdown_token.clone()
    }

    /// Handles host operating system memory pressure notifications.
    pub fn notify_memory_pressure(
        &self,
        level: MemoryPressureLevel,
        db: Option<&SqliteDb>,
    ) -> FfiResult<()> {
        self.event_sink.broadcast(BridgeEvent::MemoryPressure {
            level: level.as_str().to_string(),
        });

        // Under critical memory pressure, flush SQLite caches and checkpoint WAL
        if let Some(database) = db {
            if level == MemoryPressureLevel::Critical {
                database.with_conn(|conn| {
                    conn.execute_batch(
                        "PRAGMA shrink_memory;
                         PRAGMA wal_checkpoint(PASSIVE);",
                    )?;
                    Ok(())
                })?;
            }
        }

        Ok(())
    }

    /// Executes the two-phase graceful shutdown protocol:
    /// Phase 1: Signal cancellation to all workers, transition to `ShuttingDown`.
    /// Phase 2: Await completion of active tasks up to `timeout`, flush database, transition to `Terminated`.
    pub fn request_shutdown(
        &self,
        reason: ShutdownReason,
        timeout: Duration,
        db: Option<&SqliteDb>,
    ) -> FfiResult<()> {
        let current_state = self.state();
        if current_state == LifecycleState::Terminated {
            return Ok(());
        }

        // Phase 1: Transition to ShuttingDown and signal cancellation
        if current_state != LifecycleState::ShuttingDown {
            self.transition_to(LifecycleState::ShuttingDown, Some(reason.as_str()))?;
        }
        self.shutdown_token.cancel();

        // Phase 2: Await active worker tasks draining
        if let Ok(mut tasks) = self.tasks.lock() {
            let start = std::time::Instant::now();
            while !tasks.is_empty() {
                let elapsed = start.elapsed();
                if elapsed >= timeout {
                    break;
                }
                let remaining = timeout - elapsed;
                match self.task_condvar.wait_timeout(tasks, remaining) {
                    Ok((guard, timeout_result)) => {
                        tasks = guard;
                        if timeout_result.timed_out() {
                            break;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }

        // Flush SQLite WAL checkpoint if database handle is available
        if let Some(database) = db {
            let _ = database.with_conn(|conn| {
                conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")?;
                Ok(())
            });
        }

        // Finalize state to Terminated
        self.transition_to(LifecycleState::Terminated, Some(reason.as_str()))?;
        Ok(())
    }
}
