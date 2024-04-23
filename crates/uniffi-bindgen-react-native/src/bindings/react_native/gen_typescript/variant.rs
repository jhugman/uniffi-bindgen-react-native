use super::{AsCodeType, CodeType, CodeOracle};
use crate::interface::{ComponentInterface, Variant};

#[derive(Debug)]
pub(super) struct VariantCodeType {
    pub v: Variant,
}

impl CodeType for VariantCodeType {
    fn type_label(&self, ci: &ComponentInterface) -> String {
        CodeOracle.class_name(ci, self.v.name())
    }

    fn canonical_name(&self) -> String {
        self.v.name().to_string()
    }
}

impl AsCodeType for Variant {
    fn as_codetype(&self) -> Box<dyn CodeType> {
        Box::new(VariantCodeType { v: self.clone() })
    }
}

impl AsCodeType for &Variant {
    fn as_codetype(&self) -> Box<dyn CodeType> {
        Box::new(VariantCodeType { v: (*self).clone() })
    }
}
