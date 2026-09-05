//! Reference capabilities: statistics, geometry, dilution, and temperature.
//!
//! Typed arithmetic lives in [`super::business`]. This module owns descriptors
//! and the `Value` conversion at the capability boundary.

use axiom_rs::{
    BusinessFailure, Capability, CapabilityCategory, CapabilityDescriptor, CapabilityName,
    ExecutionContext, FieldContract, FloatRange, Object, Runtime, TypeContract, Value,
};

use super::business::{self, BoxMeasure, Dilution, Summary, TempScale};

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
        let summary = business::summarize_samples(&samples_from_value(&input)?)?;
        summary_to_value(summary)
    }
}

impl Capability for BoxVolume {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        let (x, y, z, unit) = box_from_value(&input)?;
        let measure = business::axis_aligned_box(x, y, z)?;
        box_to_value(measure, unit)
    }
}

impl Capability for Dilute {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        let stock = required_float(&input, "stock_mM")?;
        let target = required_float(&input, "target_mM")?;
        let volume = required_float(&input, "volume_mL")?;
        let result = business::dilute(stock, target, volume).map_err(|err| {
            if target > stock {
                err.with_details(object(&[
                    ("stock_mM", float_or_null(stock)),
                    ("target_mM", float_or_null(target)),
                ]))
            } else {
                err
            }
        })?;
        dilution_to_value(result)
    }
}

impl Capability for Temperature {
    fn invoke(&self, input: Value, _context: &ExecutionContext) -> Result<Value, BusinessFailure> {
        let value = required_float(&input, "value")?;
        let from = required_scale(&input, "from")?;
        let to = required_scale(&input, "to")?;
        let converted = business::convert_temperature(value, from, to)?;
        Ok(object(&[
            ("value", finite_value(converted)?),
            ("unit", Value::string(scale_name(to))),
        ]))
    }
}

fn samples_from_value(input: &Value) -> Result<Vec<i64>, BusinessFailure> {
    required_list(input, "samples")?
        .iter()
        .map(required_integer)
        .collect()
}

fn summary_to_value(summary: Summary) -> Result<Value, BusinessFailure> {
    Ok(object(&[
        ("count", Value::integer(summary.count)),
        ("min", Value::integer(summary.min)),
        ("max", Value::integer(summary.max)),
        ("mean", finite_value(summary.mean)?),
    ]))
}

fn box_from_value(input: &Value) -> Result<(f64, f64, f64, &str), BusinessFailure> {
    let size = required_object(input, "size")?;
    Ok((
        required_float_in(size, "x")?,
        required_float_in(size, "y")?,
        required_float_in(size, "z")?,
        required_str(input, "unit")?,
    ))
}

fn box_to_value(measure: BoxMeasure, unit: &str) -> Result<Value, BusinessFailure> {
    Ok(object(&[
        ("volume", finite_value(measure.volume)?),
        ("surface", finite_value(measure.surface)?),
        ("unit", Value::string(unit)),
    ]))
}

fn dilution_to_value(result: Dilution) -> Result<Value, BusinessFailure> {
    Ok(object(&[
        ("aliquot_mL", finite_value(result.aliquot_ml)?),
        ("diluent_mL", finite_value(result.diluent_ml)?),
    ]))
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

fn finite_value(value: f64) -> Result<Value, BusinessFailure> {
    Value::try_float(value).map_err(|_| BusinessFailure::new("result is not a finite number"))
}

fn float_or_null(value: f64) -> Value {
    Value::try_float(value).unwrap_or(Value::null())
}

fn required_list<'a>(input: &'a Value, field: &str) -> Result<&'a [Value], BusinessFailure> {
    let object = input.as_object().ok_or_else(missing_shape)?;
    object
        .get(field)
        .and_then(Value::as_list)
        .ok_or_else(missing_shape)
}

fn required_object<'a>(input: &'a Value, field: &str) -> Result<&'a Object, BusinessFailure> {
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

fn required_float_in(object: &Object, field: &str) -> Result<f64, BusinessFailure> {
    object
        .get(field)
        .and_then(Value::as_float)
        .ok_or_else(missing_shape)
}

fn required_integer(value: &Value) -> Result<i64, BusinessFailure> {
    value.as_integer().ok_or_else(missing_shape)
}

fn required_scale(input: &Value, field: &str) -> Result<TempScale, BusinessFailure> {
    match required_str(input, field)? {
        "C" => Ok(TempScale::C),
        "F" => Ok(TempScale::F),
        "K" => Ok(TempScale::K),
        _ => Err(missing_shape()),
    }
}

fn scale_name(scale: TempScale) -> &'static str {
    match scale {
        TempScale::C => "C",
        TempScale::F => "F",
        TempScale::K => "K",
    }
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
        let overflow = runtime
            .invoke(
                &CapabilityName::parse("stats.summarize").unwrap(),
                object(&[(
                    "samples",
                    Value::list([Value::integer(i64::MAX), Value::integer(i64::MAX)]),
                )]),
                &ctx(),
            )
            .unwrap();
        assert!(overflow.as_object().unwrap().get("mean").is_some());
    }

    #[test]
    fn box_rejects_zero_length() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let input = object(&[
            (
                "size",
                object(&[
                    ("x", finite_value(0.0).unwrap()),
                    ("y", finite_value(1.0).unwrap()),
                    ("z", finite_value(1.0).unwrap()),
                ]),
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

    #[test]
    fn box_overflow_is_business_failure() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let huge = finite_value(1e200).unwrap();
        let err = runtime
            .invoke(
                &CapabilityName::parse("geom.axis_aligned_box").unwrap(),
                object(&[
                    (
                        "size",
                        object(&[("x", huge.clone()), ("y", huge.clone()), ("z", huge)]),
                    ),
                    ("unit", Value::string("mm")),
                ]),
                &ctx(),
            )
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::BusinessFailure);
    }

    #[test]
    fn box_contract_fields_match_conversion() {
        let contract = box_input().unwrap();
        let input = object(&[
            (
                "size",
                object(&[
                    ("x", finite_value(1.0).unwrap()),
                    ("y", finite_value(2.0).unwrap()),
                    ("z", finite_value(3.0).unwrap()),
                ]),
            ),
            ("unit", Value::string("mm")),
        ]);
        contract.validate(&input).unwrap();
        let (x, y, z, unit) = box_from_value(&input).unwrap();
        assert_eq!((x, y, z, unit), (1.0, 2.0, 3.0, "mm"));
        assert_eq!(object_field_names(&contract), ["size", "unit"]);
        assert_eq!(
            object_field_names(nested(&contract, "size")),
            ["x", "y", "z"]
        );
    }

    #[test]
    fn omitted_size_w_changes_observable_volume() {
        let live = box_input().unwrap();
        let with_w = object(&[
            (
                "size",
                object(&[
                    ("x", finite_value(1.0).unwrap()),
                    ("y", finite_value(1.0).unwrap()),
                    ("z", finite_value(1.0).unwrap()),
                    ("w", finite_value(10.0).unwrap()),
                ]),
            ),
            ("unit", Value::string("mm")),
        ]);
        assert_eq!(
            live.validate(&with_w).unwrap_err().kind(),
            ErrorKind::UnknownField
        );

        let drifted = TypeContract::object(vec![
            FieldContract::new("size", drifted_size().unwrap(), true),
            FieldContract::new("unit", TypeContract::String, true)
                .with_enum_values(vec!["mm".into(), "m".into()])
                .unwrap(),
        ])
        .unwrap();
        drifted.validate(&with_w).unwrap();
        let (x, y, z, _) = box_from_value(&with_w).unwrap();
        let stale = business::axis_aligned_box(x, y, z).unwrap().volume;
        let w = required_float_in(required_object(&with_w, "size").unwrap(), "w").unwrap();
        let fixed = business::axis_aligned_box(x, y, z * w).unwrap().volume;
        assert!((stale - 1.0).abs() < f64::EPSILON);
        assert!((fixed - 10.0).abs() < f64::EPSILON);
        assert_ne!(stale, fixed);
    }

    fn object_field_names(contract: &TypeContract) -> Vec<&str> {
        contract
            .fields()
            .unwrap()
            .iter()
            .map(FieldContract::name)
            .collect()
    }

    fn nested<'a>(contract: &'a TypeContract, name: &str) -> &'a TypeContract {
        contract
            .fields()
            .unwrap()
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .contract()
    }

    fn drifted_size() -> Result<TypeContract, axiom_rs::Error> {
        TypeContract::object(vec![
            positive_length("x")?,
            positive_length("y")?,
            positive_length("z")?,
            positive_length("w")?,
        ])
        .map_err(invalid)
    }

    fn invoke_named(runtime: &Runtime, name: &str, input: Value) -> Value {
        runtime
            .invoke(&CapabilityName::parse(name).unwrap(), input, &ctx())
            .unwrap()
    }

    fn dilute_input(stock: f64, target: f64, volume: f64) -> Value {
        object(&[
            ("stock_mM", finite_value(stock).unwrap()),
            ("target_mM", finite_value(target).unwrap()),
            ("volume_mL", finite_value(volume).unwrap()),
        ])
    }

    fn field_float(value: &Value, field: &str) -> f64 {
        value
            .as_object()
            .unwrap()
            .get(field)
            .unwrap()
            .as_float()
            .unwrap()
    }

    #[test]
    fn dilute_round_trips_subnormal_and_min_normal_targets() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let min_pos = f64::from_bits(1);
        let max_subnormal = f64::from_bits((1_u64 << 52) - 1);
        let min_normal = f64::from_bits(1_u64 << 52);
        for target in [min_pos, max_subnormal, min_normal] {
            let out = invoke_named(&runtime, "chem.dilute", dilute_input(1.0, target, 1.0));
            assert_eq!(field_float(&out, "aliquot_mL"), target);
        }
        let out = invoke_named(&runtime, "chem.dilute", dilute_input(1.0, 1e-310, 1e300));
        let aliquot = field_float(&out, "aliquot_mL");
        assert!((aliquot - 1e-10).abs() / 1e-10 < 1e-10, "aliquot={aliquot}");
    }

    fn box_volume(runtime: &Runtime, x: f64, y: f64, z: f64) -> f64 {
        field_float(
            &invoke_named(
                runtime,
                "geom.axis_aligned_box",
                object(&[
                    (
                        "size",
                        object(&[
                            ("x", finite_value(x).unwrap()),
                            ("y", finite_value(y).unwrap()),
                            ("z", finite_value(z).unwrap()),
                        ]),
                    ),
                    ("unit", Value::string("mm")),
                ]),
            ),
            "volume",
        )
    }

    #[test]
    fn box_keeps_min_positive_volume_from_normal_edges() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let edge = f64::from_bits(((1023 - 537) as u64) << 52);
        assert!(edge.is_normal());
        assert_eq!(box_volume(&runtime, edge, edge, 1.0).to_bits(), 1);
    }

    #[test]
    fn box_rounds_subnormal_volume_at_midpoint_and_neighbors() {
        let runtime = Runtime::new();
        register(&runtime).unwrap();
        let edge = f64::from_bits(((1023 - 537) as u64) << 52);
        let half = f64::from_bits(0x3fe0_0000_0000_0000);
        let below = f64::from_bits(half.to_bits() - 1);
        let above = f64::from_bits(half.to_bits() + 1);
        assert_eq!(box_volume(&runtime, edge, edge, below).to_bits(), 0);
        assert_eq!(box_volume(&runtime, edge, edge, half).to_bits(), 0);
        assert_eq!(box_volume(&runtime, edge, edge, above).to_bits(), 1);
    }
}
