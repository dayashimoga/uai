//! Built-in dimensional unit conversion skill (`system.convert`).

use aissistant_orchestrator::{OrchestratorError, ToolHandler};
use aissistant_security::RiskTier;
use serde_json::Value;

/// Tool converting numeric quantities across temperature, length, mass, digital storage, and time.
#[derive(Debug, Default, Clone, Copy)]
pub struct ConvertSkill;

impl ConvertSkill {
    /// Creates a new `ConvertSkill` instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Dimension {
    Temperature,
    Length,
    Mass,
    DigitalData,
    Time,
}

impl ToolHandler for ConvertSkill {
    fn name(&self) -> &str {
        "system.convert"
    }

    fn description(&self) -> &str {
        "Converts numeric values between units across temperature, length, mass, data, and time."
    }

    fn parameters_schema(&self) -> Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "value": {
                    "type": "number",
                    "description": "Numeric magnitude to convert."
                },
                "from_unit": {
                    "type": "string",
                    "description": "Source unit of measurement (e.g. 'c', 'f', 'km', 'mi', 'kg', 'lb', 'mb', 'gb')."
                },
                "to_unit": {
                    "type": "string",
                    "description": "Target unit of measurement."
                }
            },
            "required": ["value", "from_unit", "to_unit"]
        })
    }

    fn risk_tier(&self) -> RiskTier {
        RiskTier::ReadOnly
    }

    fn execute(&self, arguments: Value) -> Result<Value, OrchestratorError> {
        let value = arguments
            .get("value")
            .and_then(Value::as_f64)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "system.convert".to_string(),
                reason: "Missing or invalid numeric 'value'".to_string(),
            })?;

        let from_unit = arguments
            .get("from_unit")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "system.convert".to_string(),
                reason: "Missing 'from_unit'".to_string(),
            })?
            .trim()
            .to_ascii_lowercase();

        let to_unit = arguments
            .get("to_unit")
            .and_then(Value::as_str)
            .ok_or_else(|| OrchestratorError::InvalidToolArguments {
                tool_name: "system.convert".to_string(),
                reason: "Missing 'to_unit'".to_string(),
            })?
            .trim()
            .to_ascii_lowercase();

        let result = convert_units(value, &from_unit, &to_unit).map_err(|err| {
            OrchestratorError::ToolExecutionFailed {
                tool_name: "system.convert".to_string(),
                reason: err,
            }
        })?;

        let formatted = if (result.fract()).abs() < 1e-9 {
            format!("{:.0}", result)
        } else {
            format!("{:.6}", result)
                .trim_end_matches('0')
                .trim_end_matches('.')
                .to_string()
        };

        Ok(serde_json::json!({
            "from_value": value,
            "from_unit": from_unit,
            "to_value": result,
            "to_unit": to_unit,
            "formatted": format!("{formatted} {to_unit}")
        }))
    }
}

fn convert_units(value: f64, from: &str, to: &str) -> Result<f64, String> {
    if from == to {
        return Ok(value);
    }

    let (dim_from, _) = parse_unit(from)?;
    let (dim_to, _) = parse_unit(to)?;

    if dim_from != dim_to {
        return Err(format!(
            "Incompatible unit conversion from '{from}' ({dim_from:?}) to '{to}' ({dim_to:?})"
        ));
    }

    match dim_from {
        Dimension::Temperature => convert_temperature(value, from, to),
        Dimension::Length | Dimension::Mass | Dimension::DigitalData | Dimension::Time => {
            let (_, factor_from) = parse_unit(from)?;
            let (_, factor_to) = parse_unit(to)?;
            let base_units = value * factor_from;
            Ok(base_units / factor_to)
        }
    }
}

fn parse_unit(unit: &str) -> Result<(Dimension, f64), String> {
    match unit {
        // Temperature
        "c" | "celsius" | "degc" | "°c" | "f" | "fahrenheit" | "degf" | "°f" | "k" | "kelvin" => {
            Ok((Dimension::Temperature, 0.0))
        }

        // Length (base: meter)
        "m" | "meter" | "meters" => Ok((Dimension::Length, 1.0)),
        "km" | "kilometer" | "kilometers" => Ok((Dimension::Length, 1000.0)),
        "cm" | "centimeter" | "centimeters" => Ok((Dimension::Length, 0.01)),
        "mm" | "millimeter" | "millimeters" => Ok((Dimension::Length, 0.001)),
        "mi" | "mile" | "miles" => Ok((Dimension::Length, 1609.344)),
        "yd" | "yard" | "yards" => Ok((Dimension::Length, 0.9144)),
        "ft" | "foot" | "feet" => Ok((Dimension::Length, 0.3048)),
        "in" | "inch" | "inches" => Ok((Dimension::Length, 0.0254)),

        // Mass (base: gram)
        "g" | "gram" | "grams" => Ok((Dimension::Mass, 1.0)),
        "kg" | "kilogram" | "kilograms" => Ok((Dimension::Mass, 1000.0)),
        "mg" | "milligram" | "milligrams" => Ok((Dimension::Mass, 0.001)),
        "lb" | "lbs" | "pound" | "pounds" => Ok((Dimension::Mass, 453.59237)),
        "oz" | "ounce" | "ounces" => Ok((Dimension::Mass, 28.349523125)),
        "ton" | "tonne" => Ok((Dimension::Mass, 1_000_000.0)),

        // Digital Data (base: byte)
        "b" | "byte" | "bytes" => Ok((Dimension::DigitalData, 1.0)),
        "kb" | "kilobyte" | "kilobytes" => Ok((Dimension::DigitalData, 1024.0)),
        "mb" | "megabyte" | "megabytes" => Ok((Dimension::DigitalData, 1024.0 * 1024.0)),
        "gb" | "gigabyte" | "gigabytes" => Ok((Dimension::DigitalData, 1024.0 * 1024.0 * 1024.0)),
        "tb" | "terabyte" | "terabytes" => {
            Ok((Dimension::DigitalData, 1024.0 * 1024.0 * 1024.0 * 1024.0))
        }

        // Time (base: second)
        "s" | "sec" | "second" | "seconds" => Ok((Dimension::Time, 1.0)),
        "ms" | "millisecond" | "milliseconds" => Ok((Dimension::Time, 0.001)),
        "min" | "minute" | "minutes" => Ok((Dimension::Time, 60.0)),
        "h" | "hr" | "hour" | "hours" => Ok((Dimension::Time, 3600.0)),
        "d" | "day" | "days" => Ok((Dimension::Time, 86400.0)),
        "wk" | "week" | "weeks" => Ok((Dimension::Time, 604800.0)),

        other => Err(format!("Unknown unit of measurement: '{other}'")),
    }
}

fn convert_temperature(val: f64, from: &str, to: &str) -> Result<f64, String> {
    // Convert source to Celsius first
    let celsius = match from {
        "c" | "celsius" | "degc" | "°c" => val,
        "f" | "fahrenheit" | "degf" | "°f" => (val - 32.0) * 5.0 / 9.0,
        "k" | "kelvin" => val - 273.15,
        other => return Err(format!("Unknown temperature unit: '{other}'")),
    };

    // Convert Celsius to target
    match to {
        "c" | "celsius" | "degc" | "°c" => Ok(celsius),
        "f" | "fahrenheit" | "degf" | "°f" => Ok(celsius * 9.0 / 5.0 + 32.0),
        "k" | "kelvin" => Ok(celsius + 273.15),
        other => Err(format!("Unknown temperature unit: '{other}'")),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_temperature_conversion() {
        // 100 C = 212 F
        let f = convert_units(100.0, "c", "f").unwrap();
        assert!((f - 212.0).abs() < 1e-6);

        // 32 F = 0 C
        let c = convert_units(32.0, "f", "c").unwrap();
        assert!((c - 0.0).abs() < 1e-6);

        // 0 C = 273.15 K
        let k = convert_units(0.0, "c", "k").unwrap();
        assert!((k - 273.15).abs() < 1e-6);
    }

    #[test]
    fn test_length_conversion() {
        // 1 mile ~ 1.609344 km
        let km = convert_units(1.0, "mi", "km").unwrap();
        assert!((km - 1.609344).abs() < 1e-6);

        // 100 cm = 1 m
        let m = convert_units(100.0, "cm", "m").unwrap();
        assert_eq!(m, 1.0);
    }

    #[test]
    fn test_digital_data_conversion() {
        // 1024 MB = 1 GB
        let gb = convert_units(1024.0, "mb", "gb").unwrap();
        assert_eq!(gb, 1.0);
    }

    #[test]
    fn test_incompatible_units_rejected() {
        assert!(convert_units(10.0, "kg", "km").is_err());
        assert!(convert_units(5.0, "c", "mb").is_err());
    }

    #[test]
    fn test_convert_skill_handler() {
        let skill = ConvertSkill::new();
        assert_eq!(skill.name(), "system.convert");
        assert_eq!(skill.risk_tier(), RiskTier::ReadOnly);

        let res = skill
            .execute(serde_json::json!({
                "value": 1.0,
                "from_unit": "km",
                "to_unit": "m"
            }))
            .unwrap();
        assert_eq!(res["to_value"], 1000.0);
        assert_eq!(res["formatted"], "1000 m");
    }
}
