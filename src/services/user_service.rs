#[derive(Debug, Clone, Default)]
pub struct UserService;

impl UserService {
    pub fn new() -> Self {
        Self
    }

    pub fn greeting(&self, name: &str) -> String {
        format!("Привет, {}!", name)
    }
}
