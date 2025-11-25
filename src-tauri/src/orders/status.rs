#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum OrderStage {
    Code(u8)
}

impl OrderStage {
    pub fn from_code(code: u8) -> Self {
        OrderStage::Code(code)
    }
    pub fn label(&self) -> String {
        match self {
            OrderStage::Code(c) => format!("Code{}", c)
        }
    }
}