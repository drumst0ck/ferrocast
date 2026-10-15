//! Unit conversion table for the calculator (`10ft in m`, `2rem in px`).

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dim {
    Length,
    Mass,
    Time,
    Data,
    Volume,
    Area,
    Speed,
    Temperature,
    Css,
    Angle,
}

#[derive(Debug, Clone, Copy)]
pub struct Unit {
    pub name: &'static str,
    pub symbol: &'static str,
    pub dim: Dim,
    /// Factor to the dimension's base unit (temperature uses `offset` too).
    pub factor: f64,
    pub offset: f64,
    pub aliases: &'static [&'static str],
}

macro_rules! u {
    ($name:expr, $sym:expr, $dim:ident, $f:expr, [$($a:expr),*]) => {
        Unit { name: $name, symbol: $sym, dim: Dim::$dim, factor: $f, offset: 0.0, aliases: &[$($a),*] }
    };
}

pub static UNITS: &[Unit] = &[
    // Length (base: metre)
    u!("millimeter", "mm", Length, 0.001, ["mm", "millimeter", "millimeters", "millimetre", "millimetres"]),
    u!("centimeter", "cm", Length, 0.01, ["cm", "centimeter", "centimeters", "centimetre", "centimetres"]),
    u!("meter", "m", Length, 1.0, ["m", "meter", "meters", "metre", "metres"]),
    u!("kilometer", "km", Length, 1000.0, ["km", "kilometer", "kilometers", "kilometre", "kilometres"]),
    u!("inch", "in", Length, 0.0254, ["inch", "inches", "\""]),
    u!("foot", "ft", Length, 0.3048, ["ft", "foot", "feet", "'"]),
    u!("yard", "yd", Length, 0.9144, ["yd", "yard", "yards"]),
    u!("mile", "mi", Length, 1609.344, ["mi", "mile", "miles"]),
    u!("nautical mile", "nmi", Length, 1852.0, ["nmi", "nautical mile", "nautical miles"]),
    // Mass (base: kilogram)
    u!("milligram", "mg", Mass, 1e-6, ["mg", "milligram", "milligrams"]),
    u!("gram", "g", Mass, 0.001, ["g", "gram", "grams"]),
    u!("kilogram", "kg", Mass, 1.0, ["kg", "kilogram", "kilograms", "kilo", "kilos"]),
    u!("tonne", "t", Mass, 1000.0, ["t", "tonne", "tonnes", "ton", "tons"]),
    u!("ounce", "oz", Mass, 0.028349523125, ["oz", "ounce", "ounces"]),
    u!("pound", "lb", Mass, 0.45359237, ["lb", "lbs", "pound", "pounds"]),
    u!("stone", "st", Mass, 6.35029318, ["stone", "stones"]),
    // Time (base: second)
    u!("millisecond", "ms", Time, 0.001, ["ms", "millisecond", "milliseconds"]),
    u!("second", "s", Time, 1.0, ["s", "sec", "secs", "second", "seconds"]),
    u!("minute", "min", Time, 60.0, ["min", "mins", "minute", "minutes"]),
    u!("hour", "h", Time, 3600.0, ["h", "hr", "hrs", "hour", "hours"]),
    u!("day", "d", Time, 86400.0, ["d", "day", "days"]),
    u!("week", "wk", Time, 604800.0, ["wk", "week", "weeks"]),
    u!("month", "mo", Time, 2629800.0, ["mo", "month", "months"]),
    u!("year", "yr", Time, 31557600.0, ["yr", "yrs", "year", "years"]),
    // Data (base: byte)
    u!("bit", "bit", Data, 0.125, ["bit", "bits"]),
    u!("byte", "B", Data, 1.0, ["b", "byte", "bytes"]),
    u!("kilobyte", "KB", Data, 1e3, ["kb", "kilobyte", "kilobytes"]),
    u!("megabyte", "MB", Data, 1e6, ["mb", "megabyte", "megabytes"]),
    u!("gigabyte", "GB", Data, 1e9, ["gb", "gigabyte", "gigabytes"]),
    u!("terabyte", "TB", Data, 1e12, ["tb", "terabyte", "terabytes"]),
    u!("kibibyte", "KiB", Data, 1024.0, ["kib", "kibibyte", "kibibytes"]),
    u!("mebibyte", "MiB", Data, 1048576.0, ["mib", "mebibyte", "mebibytes"]),
    u!("gibibyte", "GiB", Data, 1073741824.0, ["gib", "gibibyte", "gibibytes"]),
    // Volume (base: litre)
    u!("milliliter", "ml", Volume, 0.001, ["ml", "milliliter", "milliliters", "millilitre"]),
    u!("liter", "l", Volume, 1.0, ["l", "liter", "liters", "litre", "litres"]),
    u!("gallon", "gal", Volume, 3.785411784, ["gal", "gallon", "gallons"]),
    u!("quart", "qt", Volume, 0.946352946, ["qt", "quart", "quarts"]),
    u!("pint", "pt", Volume, 0.473176473, ["pint", "pints"]),
    u!("cup", "cup", Volume, 0.2365882365, ["cup", "cups"]),
    u!("fluid ounce", "fl oz", Volume, 0.0295735295625, ["fl oz", "floz"]),
    u!("tablespoon", "tbsp", Volume, 0.01478676478125, ["tbsp", "tablespoon", "tablespoons"]),
    u!("teaspoon", "tsp", Volume, 0.00492892159375, ["tsp", "teaspoon", "teaspoons"]),
    // Area (base: square metre)
    u!("square meter", "m²", Area, 1.0, ["m2", "m²", "sqm", "square meter", "square meters"]),
    u!("square kilometer", "km²", Area, 1e6, ["km2", "km²", "square kilometer", "square kilometers"]),
    u!("square foot", "ft²", Area, 0.09290304, ["ft2", "ft²", "sqft", "square foot", "square feet"]),
    u!("hectare", "ha", Area, 10000.0, ["ha", "hectare", "hectares"]),
    u!("acre", "ac", Area, 4046.8564224, ["ac", "acre", "acres"]),
    // Speed (base: m/s)
    u!("meter per second", "m/s", Speed, 1.0, ["m/s", "mps"]),
    u!("kilometer per hour", "km/h", Speed, 1.0 / 3.6, ["km/h", "kmh", "kph"]),
    u!("mile per hour", "mph", Speed, 0.44704, ["mph", "mi/h"]),
    u!("knot", "kn", Speed, 0.514444, ["kn", "knot", "knots"]),
    // CSS (base: px; rem/em factor is configurable at runtime)
    u!("pixel", "px", Css, 1.0, ["px", "pixel", "pixels"]),
    u!("rem", "rem", Css, 16.0, ["rem"]),
    u!("em", "em", Css, 16.0, ["em"]),
    u!("point", "pt", Css, 4.0 / 3.0, ["pt", "point", "points"]),
    // Angle (base: radian)
    u!("radian", "rad", Angle, 1.0, ["rad", "radian", "radians"]),
    u!("degree", "°", Angle, std::f64::consts::PI / 180.0, ["deg", "degree", "degrees", "°"]),
    // Temperature (base: kelvin) — factor/offset: K = v*factor + offset
    Unit { name: "celsius", symbol: "°C", dim: Dim::Temperature, factor: 1.0, offset: 273.15, aliases: &["c", "°c", "celsius", "degc"] },
    Unit {
        name: "fahrenheit",
        symbol: "°F",
        dim: Dim::Temperature,
        factor: 5.0 / 9.0,
        offset: 459.67 * 5.0 / 9.0,
        aliases: &["f", "°f", "fahrenheit", "degf"],
    },
    Unit { name: "kelvin", symbol: "K", dim: Dim::Temperature, factor: 1.0, offset: 0.0, aliases: &["k", "kelvin"] },
];

/// Look up a unit by alias (case-insensitive).
pub fn find(alias: &str) -> Option<&'static Unit> {
    let a = alias.trim().to_lowercase();
    if a.is_empty() {
        return None;
    }
    UNITS.iter().find(|u| u.aliases.iter().any(|x| *x == a))
}

/// All aliases sorted longest-first (for suffix matching).
pub fn aliases_longest_first() -> Vec<(&'static str, &'static Unit)> {
    let mut v: Vec<_> = UNITS.iter().flat_map(|u| u.aliases.iter().map(move |a| (*a, u))).collect();
    v.sort_by_key(|a| std::cmp::Reverse(a.0.len()));
    v
}

/// Convert `value` from one unit to another; `rem_px` overrides rem/em size.
pub fn convert(value: f64, from: &Unit, to: &Unit, rem_px: f64) -> Option<f64> {
    if from.dim != to.dim {
        return None;
    }
    let f = |u: &Unit| if u.dim == Dim::Css && (u.name == "rem" || u.name == "em") { rem_px } else { u.factor };
    let base = value * f(from) + from.offset;
    Some((base - to.offset) / f(to))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(v: f64, a: &str, b: &str) -> f64 {
        convert(v, find(a).unwrap(), find(b).unwrap(), 16.0).unwrap()
    }

    #[test]
    fn length() {
        assert!((conv(10.0, "ft", "m") - 3.048).abs() < 1e-9);
        assert!((conv(1.0, "mile", "km") - 1.609344).abs() < 1e-9);
    }

    #[test]
    fn temperature() {
        assert!((conv(100.0, "c", "f") - 212.0).abs() < 1e-9);
        assert!((conv(32.0, "f", "c")).abs() < 1e-9);
        assert!((conv(0.0, "c", "k") - 273.15).abs() < 1e-9);
    }

    #[test]
    fn css() {
        assert!((conv(2.0, "rem", "px") - 32.0).abs() < 1e-9);
    }

    #[test]
    fn incompatible() {
        assert!(convert(1.0, find("kg").unwrap(), find("m").unwrap(), 16.0).is_none());
    }
}
