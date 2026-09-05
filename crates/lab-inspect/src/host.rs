//! Reference capabilities: statistics, geometry, dilution, and temperature.

use axiom_rs::{
    BusinessFailure, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName,
    ExecutionContext, FieldContract, FloatRange, Runtime, TypeContract, Value,
};

/// Marker type for the reference host.
#[derive(Clone, Debug, Default)]
pub struct LabHost;

/// Register the four reference capabilities on `runtime`.
///
/// # Errors
///
/// Propagates duplicate registration errors.
pub fn register(runtime: &Runtime) -> Result<(), axiom_rs::Error> {
    runtime.register(stats_descriptor()?, Stats)?;
    runtime.register(box_descriptor()?, BoxVolume)?;
    runtime.register(dilute_descriptor()?, Dilute)?;
    runtime.register(temp_descriptor()?, Temperature)?;
    Ok(())
}

struct Stats;
struct BoxVolume;
struct Dilute;
struct Temperature;

impl Capability for Stats {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        summarize(input)
    }
}

impl Capability for BoxVolume {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        box_metrics(input)
    }
}

impl Capability for Dilute {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        dilute(input)
    }
}

impl Capability for Temperature {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        convert_temp(input)
    }
}

fn summarize(input: Value) -> Result<Value, BusinessFailure> {
    let samples = required_list(&input, "samples")?;
    if samples.is_empty() {
        return Err(BusinessFailure::new("samples must not be empty"));
    }
    let mut values = Vec::new();
    for sample in samples {
        values.push(required_integer(sample)?);
    }
    let count = i64::try_from(values.len()).expect("sample count fits i64");
    let min = *values.iter().min().expect("non-empty");
    let max = *values.iter().max().expect("non-empty");
    let sum: i64 = values.iter().sum();
    let mean = (sum as f64) / (count as f64);
    Ok(object(&[
        ("count", Value::integer(count)),
        ("min", Value::integer(min)),
        ("max", Value::integer(max)),
        ("mean", float(mean)),
    ]))
}

fn box_metrics(input: Value) -> Result<Value, BusinessFailure> {
    let size = required_object(&input, "size")?;
    let x = required_float_in(size, "x")?;
    let y = required_float_in(size, "y")?;
    let z = required_float_in(size, "z")?;
    let unit = required_str(&input, "unit")?;
    let volume = x * y * z;
    let surface = 2.0 * (x * y + y * z + z * x);
    Ok(object(&[
        ("volume", float(volume)),
        ("surface", float(surface)),
        ("unit", Value::string(unit)),
    ]))
}

fn dilute(input: Value) -> Result<Value, BusinessFailure> {
    let stock = required_float(&input, "stock_mM")?;
    let target = required_float(&input, "target_mM")?;
    let volume = required_float(&input, "volume_mL")?;
    if target > stock {
        return Err(
            BusinessFailure::new("target exceeds stock concentration").with_details(object(&[
                ("stock_mM", float(stock)),
                ("target_mM", float(target)),
            ])),
        );
    }
    let aliquot = volume * target / stock;
    let diluent = volume - aliquot;
    Ok(object(&[
        ("aliquot_mL", float(aliquot)),
        ("diluent_mL", float(diluent)),
    ]))
}

fn convert_temp(input: Value) -> Result<Value, BusinessFailure> {
    let value = required_float(&input, "value")?;
    let from = required_str(&input, "from")?;
    let to = required_str(&input, "to")?;
    let kelvin = to_kelvin(value, from)?;
    if kelvin < 0.0 {
        return Err(BusinessFailure::new("temperature is below absolute zero"));
    }
    let converted = from_kelvin(kelvin, to)?;
    Ok(object(&[
        ("value", float(converted)),
        ("unit", Value::string(to)),
    ]))
}

fn to_kelvin(value: f64, unit: &str) -> Result<f64, BusinessFailure> {
    match unit {
        "K" => Ok(value),
        "C" => Ok(value + 273.15),
        "F" => Ok((value - 32.0) * 5.0 / 9.0 + 273.15),
        _ => Err(BusinessFailure::new("unsupported temperature unit")),
    }
}

fn from_kelvin(kelvin: f64, unit: &str) -> Result<f64, BusinessFailure> {
    match unit {
        "K" => Ok(kelvin),
        "C" => Ok(kelvin - 273.15),
        "F" => Ok((kelvin - 273.15) * 9.0 / 5.0 + 32.0),
        _ => Err(BusinessFailure::new("unsupported temperature unit")),
    }
}

fn stats_descriptor() -> Result<CapabilityDescriptor, axiom_rs::Error> {
    let input = TypeContract::object(vec![
        FieldContract::new("samples", TypeContract::list(TypeContract::Integer), true)
            .with_description("sample values to summarize")
            .map_err(invalid)?,
    ])
    .map_err(invalid)?;
    let output = TypeContract::object(vec![
        described("count", TypeContract::Integer, "number of samples")?,
        described("min", TypeContract::Integer, "minimum sample")?,
        described("max", TypeContract::Integer, "maximum sample")?,
        described("mean", TypeContract::Float, "arithmetic mean")?,
    ])
    .map_err(invalid)?;
    descriptor(
        "stats.summarize",
        "summarize integer samples",
        input,
        output,
    )
}

fn box_descriptor() -> Result<CapabilityDescriptor, axiom_rs::Error> {
    let input = box_input()?;
    let output = TypeContract::object(vec![
        described("volume", TypeContract::Float, "axis-aligned volume")?,
        described("surface", TypeContract::Float, "surface area")?,
        described("unit", TypeContract::String, "unit of volume and area")?,
    ])
    .map_err(invalid)?;
    descriptor(
        "geom.axis_aligned_box",
        "volume and surface of an axis-aligned box",
        input,
        output,
    )
}

fn box_input() -> Result<TypeContract, axiom_rs::Error> {
    let size = TypeContract::object(vec![
        positive_length("x")?,
        positive_length("y")?,
        positive_length("z")?,
    ])
    .map_err(invalid)?;
    TypeContract::object(vec![
        FieldContract::new("size", size, true)
            .with_description("edge lengths")
            .map_err(invalid)?,
        FieldContract::new("unit", TypeContract::String, true)
            .with_description("length unit")
            .map_err(invalid)?
            .with_enum_values(vec!["mm".into(), "m".into()])
            .map_err(invalid)?,
    ])
    .map_err(invalid)
}

fn dilute_descriptor() -> Result<CapabilityDescriptor, axiom_rs::Error> {
    let input = TypeContract::object(vec![
        positive_conc("stock_mM", "stock concentration")?,
        positive_conc("target_mM", "desired concentration")?,
        positive_conc("volume_mL", "final volume")?,
    ])
    .map_err(invalid)?;
    let output = TypeContract::object(vec![
        described("aliquot_mL", TypeContract::Float, "stock volume")?,
        described("diluent_mL", TypeContract::Float, "diluent volume")?,
    ])
    .map_err(invalid)?;
    descriptor("chem.dilute", "compute a dilution", input, output)
}

fn temp_descriptor() -> Result<CapabilityDescriptor, axiom_rs::Error> {
    let input = TypeContract::object(vec![
        described("value", TypeContract::Float, "temperature magnitude")?,
        enum_unit("from", "source scale")?,
        enum_unit("to", "target scale")?,
    ])
    .map_err(invalid)?;
    let output = TypeContract::object(vec![
        described("value", TypeContract::Float, "converted temperature")?,
        described("unit", TypeContract::String, "output scale")?,
    ])
    .map_err(invalid)?;
    descriptor("temp.convert", "convert temperature scales", input, output)
}

fn positive_length(name: &str) -> Result<FieldContract, axiom_rs::Error> {
    FieldContract::new(name, TypeContract::Float, true)
        .with_description("edge length")
        .map_err(invalid)?
        .with_unit("length")
        .map_err(invalid)?
        .with_float_range(FloatRange {
            min: Some(0.0),
            max: None,
            min_exclusive: true,
            max_exclusive: false,
        })
        .map_err(invalid)
}

fn positive_conc(name: &str, description: &str) -> Result<FieldContract, axiom_rs::Error> {
    FieldContract::new(name, TypeContract::Float, true)
        .with_description(description)
        .map_err(invalid)?
        .with_float_range(FloatRange {
            min: Some(0.0),
            max: None,
            min_exclusive: true,
            max_exclusive: false,
        })
        .map_err(invalid)
}

fn enum_unit(name: &str, description: &str) -> Result<FieldContract, axiom_rs::Error> {
    FieldContract::new(name, TypeContract::String, true)
        .with_description(description)
        .map_err(invalid)?
        .with_enum_values(vec!["C".into(), "F".into(), "K".into()])
        .map_err(invalid)
}

fn described(
    name: &str,
    contract: TypeContract,
    description: &str,
) -> Result<FieldContract, axiom_rs::Error> {
    FieldContract::new(name, contract, true)
        .with_description(description)
        .map_err(invalid)
}

fn descriptor(
    name: &str,
    description: &str,
    input: TypeContract,
    output: TypeContract,
) -> Result<CapabilityDescriptor, axiom_rs::Error> {
    Ok(CapabilityDescriptor::new(
        CapabilityName::parse(name).expect("static names are valid"),
        description,
        CapabilityCategory::parse("lab").expect("static category is valid"),
        input,
        output,
    )
    .expect("static descriptions are valid"))
}

fn invalid(error: axiom_rs::InvalidContract) -> axiom_rs::Error {
    panic!("reference host descriptors must be valid: {error}")
}

fn object(fields: &[(&str, Value)]) -> Value {
    Value::try_object(fields.iter().cloned()).expect("unique keys")
}

fn float(value: f64) -> Value {
    Value::try_float(value).expect("host floats are finite")
}

fn required_list<'a>(input: &'a Value, field: &str) -> Result<&'a [Value], BusinessFailure> {
    let object = input.as_object().ok_or_else(missing_shape)?;
    object
        .get(field)
        .and_then(Value::as_list)
        .ok_or_else(missing_shape)
}

fn required_object<'a>(
    input: &'a Value,
    field: &str,
) -> Result<&'a axiom_rs::Object, BusinessFailure> {
    let object = input.as_object().ok_or_else(missing_shape)?;
    object
        .get(field)
        .and_then(Value::as_object)
        .ok_or_else(missing_shape)
}

fn required_str<'a>(input: &'a Value, field: &str) -> Result<&'a str, BusinessFailure> {
    let object = input.as_object().ok_or_else(missing_shape)?;
    object
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(missing_shape)
}

fn required_float(input: &Value, field: &str) -> Result<f64, BusinessFailure> {
    let object = input.as_object().ok_or_else(missing_shape)?;
    required_float_in(object, field)
}

fn required_float_in(object: &axiom_rs::Object, field: &str) -> Result<f64, BusinessFailure> {
    object
        .get(field)
        .and_then(Value::as_float)
        .ok_or_else(missing_shape)
}

fn required_integer(value: &Value) -> Result<i64, BusinessFailure> {
    value.as_integer().ok_or_else(missing_shape)
}

fn missing_shape() -> BusinessFailure {
    BusinessFailure::new("validated input missing expected shape")
}

#[cfg(test)]
mod tests {
    use super::*;
    use axiom_rs::{CorrelationId, ErrorKind};

    fn ctx() -> ExecutionContext {
        ExecutionContext::root(CorrelationId::parse("t").unwrap())
    }

    #[test]
    fn summarize_and_empty_failure() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let input = object(&[(
            "samples",
            Value::list([Value::integer(1), Value::integer(3)]),
        )]);
        let out = runtime
            .invoke(
                &CapabilityName::parse("stats.summarize").unwrap(),
                input,
                &ctx(),
            )
            .unwrap();
        assert_eq!(
            out.as_object().unwrap().get("min"),
            Some(&Value::integer(1))
        );
        let err = runtime
            .invoke(
                &CapabilityName::parse("stats.summarize").unwrap(),
                object(&[("samples", Value::list([]))]),
                &ctx(),
            )
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BusinessFailure);
    }

    #[test]
    fn box_rejects_zero_length() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let input = object(&[
            (
                "size",
                object(&[("x", float(0.0)), ("y", float(1.0)), ("z", float(1.0))]),
            ),
            ("unit", Value::string("mm")),
        ]);
        let err = runtime
            .invoke(
                &CapabilityName::parse("geom.axis_aligned_box").unwrap(),
                input,
                &ctx(),
            )
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ConstraintViolation);
    }
}
