package com.aissistant.aissistant

import android.accessibilityservice.AccessibilityService
import android.view.accessibility.AccessibilityEvent
import android.view.accessibility.AccessibilityNodeInfo

/**
 * Opt-in Android Accessibility Service enabling offline assistant screen-reading,
 * UI node inspection, and user-authorized action clicks.
 *
 * All operations are strictly local and on-device (zero cloud telemetry).
 */
class AssistantAccessibilityService : AccessibilityService() {

    companion object {
        @Volatile
        var instance: AssistantAccessibilityService? = null
            private set

        val isRunning: Boolean
            get() = instance != null
    }

    override fun onServiceConnected() {
        super.onServiceConnected()
        instance = this
    }

    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        // Event received; live tracking handled on-demand via rootInActiveWindow
    }

    override fun onInterrupt() {
        // Interrupt signal from system
    }

    override fun onDestroy() {
        instance = null
        super.onDestroy()
    }

    /**
     * Traverses the active window node hierarchy up to [maxDepth] and serializes
     * visible node attributes for assistant understanding.
     */
    fun getActiveWindowHierarchy(maxDepth: Int = 4): Map<String, Any?>? {
        val root = rootInActiveWindow ?: return null
        val result = serializeNode(root, 0, maxDepth)
        @Suppress("DEPRECATION")
        root.recycle()
        return result
    }

    private fun serializeNode(
        node: AccessibilityNodeInfo,
        depth: Int,
        maxDepth: Int
    ): Map<String, Any?> {
        val map = mutableMapOf<String, Any?>()

        map["packageName"] = node.packageName?.toString()
        map["className"] = node.className?.toString()
        map["text"] = node.text?.toString()
        map["contentDescription"] = node.contentDescription?.toString()
        map["viewId"] = node.viewIdResourceName
        map["isClickable"] = node.isClickable
        map["isEnabled"] = node.isEnabled

        if (depth < maxDepth && node.childCount > 0) {
            val children = mutableListOf<Map<String, Any?>>()
            val childCount = minOf(node.childCount, 20) // Cap to avoid unbounded tree
            for (i in 0 until childCount) {
                val child = node.getChild(i)
                if (child != null) {
                    children.add(serializeNode(child, depth + 1, maxDepth))
                    @Suppress("DEPRECATION")
                    child.recycle()
                }
            }
            map["children"] = children
        }

        return map
    }

    /**
     * Searches for an interactive node whose text or content description matches [targetText]
     * and performs an ACTION_CLICK gesture.
     */
    fun clickNodeByText(targetText: String): Boolean {
        val root = rootInActiveWindow ?: return false
        val nodes = root.findAccessibilityNodeInfosByText(targetText)

        var clicked = false
        for (node in nodes) {
            if (!clicked) {
                if (node.isClickable) {
                    clicked = node.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                } else {
                    var parent = node.parent
                    while (parent != null && !clicked) {
                        if (parent.isClickable) {
                            clicked = parent.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                        }
                        val nextParent = parent.parent
                        @Suppress("DEPRECATION")
                        parent.recycle()
                        parent = nextParent
                    }
                }
            }
            @Suppress("DEPRECATION")
            node.recycle()
        }

        @Suppress("DEPRECATION")
        root.recycle()
        return clicked
    }
}
