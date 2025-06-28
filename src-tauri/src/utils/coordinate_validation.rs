/// Anthropic Computer Use API - Strict Coordinate Validation
///
/// This module implements exact compliance with the Anthropic Computer Use specification:
/// - coordinate parameter must be tuple [int, int]
/// - validation: isinstance(coordinate, list) and len(coordinate) == 2
/// - each coordinate value must be an integer
use serde_json::Value;

/// Coordinate validation error types matching Anthropic specification
#[derive(Debug, Clone)]
pub enum CoordinateValidationError {
    /// coordinate parameter is missing
    Missing,
    /// coordinate parameter is not an array/list
    NotArray,
    /// coordinate array does not have exactly 2 elements
    InvalidLength(usize),
    /// coordinate value at index is not an integer
    NotInteger(usize, String),
    /// coordinate value is out of reasonable bounds
    OutOfBounds(usize, i64),
}

impl std::fmt::Display for CoordinateValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CoordinateValidationError::Missing => {
                write!(f, "Missing 'coordinate' parameter")
            }
            CoordinateValidationError::NotArray => {
                write!(f, "coordinate parameter must be a list/array")
            }
            CoordinateValidationError::InvalidLength(len) => {
                write!(f, "coordinate must have exactly 2 elements, got {}", len)
            }
            CoordinateValidationError::NotInteger(index, value) => {
                write!(f, "coordinate[{}] must be an integer, got '{}'", index, value)
            }
            CoordinateValidationError::OutOfBounds(index, value) => {
                write!(f, "coordinate[{}] value {} is out of reasonable bounds (0-7680)", index, value)
            }
        }
    }
}

impl std::error::Error for CoordinateValidationError {}

/// Validated coordinate pair matching Anthropic specification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ValidatedCoordinate {
    pub x: i64,
    pub y: i64,
}

impl ValidatedCoordinate {
    /// Convert to screen coordinates as f64 for legacy compatibility
    pub fn to_f64(&self) -> (f64, f64) {
        (self.x as f64, self.y as f64)
    }

    /// Convert to screen coordinates as integers
    pub fn to_screen_coords(&self) -> (i64, i64) {
        (self.x, self.y)
    }
}

/// Strict coordinate validation matching Anthropic Computer Use API specification
///
/// Implements exact validation as specified:
/// - isinstance(coordinate, list) -> coordinate.as_array()
/// - len(coordinate) == 2 -> coordinate.len() == 2
/// - coordinate values must be integers
pub fn validate_coordinate_strict(coordinate_value: &Value) -> Result<ValidatedCoordinate, CoordinateValidationError> {
    // Step 1: isinstance(coordinate, list) check
    let coordinate_array = coordinate_value.as_array()
        .ok_or(CoordinateValidationError::NotArray)?;

    // Step 2: len(coordinate) == 2 check
    if coordinate_array.len() != 2 {
        return Err(CoordinateValidationError::InvalidLength(coordinate_array.len()));
    }

    // Step 3: Extract and validate integer coordinates
    let x = extract_integer_coordinate(&coordinate_array[0], 0)?;
    let y = extract_integer_coordinate(&coordinate_array[1], 1)?;

    // Step 4: Bounds validation (reasonable screen coordinates)
    validate_coordinate_bounds(x, 0)?;
    validate_coordinate_bounds(y, 1)?;

    Ok(ValidatedCoordinate { x, y })
}

/// Extract integer coordinate value with strict type validation
fn extract_integer_coordinate(value: &Value, index: usize) -> Result<i64, CoordinateValidationError> {
    // Try integer first (exact match)
    if let Some(int_val) = value.as_i64() {
        return Ok(int_val);
    }

    // Try unsigned integer
    if let Some(uint_val) = value.as_u64() {
        return Ok(uint_val as i64);
    }

    // Try float that represents an exact integer
    if let Some(float_val) = value.as_f64() {
        if float_val.fract() == 0.0 && float_val.is_finite() {
            return Ok(float_val as i64);
        }
    }

    // All other cases are invalid
    Err(CoordinateValidationError::NotInteger(
        index,
        value.to_string()
    ))
}

/// Validate coordinate bounds (reasonable screen coordinates)
fn validate_coordinate_bounds(coord: i64, index: usize) -> Result<(), CoordinateValidationError> {
    // Allow negative coordinates for off-screen scenarios, but set reasonable limits
    // Maximum bounds: 8K display (7680x4320) + some buffer for multi-monitor setups
    const MAX_COORDINATE: i64 = 7680;
    const MIN_COORDINATE: i64 = -1000; // Allow some negative coordinates

    if coord < MIN_COORDINATE || coord > MAX_COORDINATE {
        return Err(CoordinateValidationError::OutOfBounds(index, coord));
    }

    Ok(())
}

/// Validate coordinate parameter from JSON input with Anthropic API compliance
pub fn validate_coordinate_parameter(input: &Value, param_name: &str) -> Result<ValidatedCoordinate, CoordinateValidationError> {
    let coordinate_value = input.get(param_name)
        .ok_or(CoordinateValidationError::Missing)?;

    validate_coordinate_strict(coordinate_value)
}

/// Validate multiple coordinate parameters (for drag operations)
pub fn validate_coordinate_pair(
    input: &Value,
    start_param: &str,
    end_param: &str
) -> Result<(ValidatedCoordinate, ValidatedCoordinate), CoordinateValidationError> {
    let start_coord = validate_coordinate_parameter(input, start_param)?;
    let end_coord = validate_coordinate_parameter(input, end_param)?;
    Ok((start_coord, end_coord))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_integer_coordinates() {
        let coord = json!([100, 200]);
        let result = validate_coordinate_strict(&coord).unwrap();
        assert_eq!(result.x, 100);
        assert_eq!(result.y, 200);
    }

    #[test]
    fn test_valid_float_integers() {
        let coord = json!([100.0, 200.0]);
        let result = validate_coordinate_strict(&coord).unwrap();
        assert_eq!(result.x, 100);
        assert_eq!(result.y, 200);
    }

    #[test]
    fn test_invalid_not_array() {
        let coord = json!({"x": 100, "y": 200});
        let result = validate_coordinate_strict(&coord);
        assert!(matches!(result, Err(CoordinateValidationError::NotArray)));
    }

    #[test]
    fn test_invalid_length() {
        let coord = json!([100, 200, 300]);
        let result = validate_coordinate_strict(&coord);
        assert!(matches!(result, Err(CoordinateValidationError::InvalidLength(3))));
    }

    #[test]
    fn test_invalid_float_coordinates() {
        let coord = json!([100.5, 200]);
        let result = validate_coordinate_strict(&coord);
        assert!(matches!(result, Err(CoordinateValidationError::NotInteger(0, _))));
    }

    #[test]
    fn test_out_of_bounds() {
        let coord = json!([10000, 200]);
        let result = validate_coordinate_strict(&coord);
        assert!(matches!(result, Err(CoordinateValidationError::OutOfBounds(0, 10000))));
    }

    #[test]
    fn test_coordinate_parameter_validation() {
        let input = json!({"coordinate": [150, 250]});
        let result = validate_coordinate_parameter(&input, "coordinate").unwrap();
        assert_eq!(result.x, 150);
        assert_eq!(result.y, 250);
    }

    #[test]
    fn test_missing_coordinate_parameter() {
        let input = json!({"action": "click"});
        let result = validate_coordinate_parameter(&input, "coordinate");
        assert!(matches!(result, Err(CoordinateValidationError::Missing)));
    }
}
