pub const GREETING: &str =
    include_str!("greeting.txt");

pub const ANY: &str =
    include_str!("any.txt");

pub const VAR: &str =
    include_str!("variables.txt");

pub const WANT: &str =
    include_str!("want.txt");

pub const CONT: &str =
    include_str!("contacts.txt");

pub const WED: &str =
    include_str!("wednesday.txt");

pub const SAT: &str =
    include_str!("saturday.txt");





pub fn get_message(name: &str) -> Option<&'static str> {
    match name {
        "GREETING" => Some(GREETING),
        "ANY" => Some(ANY),
        "VAR" => Some(VAR),
        "WANT" => Some(WANT),
        "CONT" => Some(CONT),
        "WED" => Some(WED),
        "SAT" => Some(SAT),
    
        _ => None,
    }
}