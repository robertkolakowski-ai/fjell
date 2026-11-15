//! Kjøregrensesnitt mot ONNX-runtime: `OnnxSession` (én verifisert modell) og `SessionFactory`
//! (åpner økter fra en `VerifiedModel`). `MockSession` brukes i tester og til integrasjon uten runtime.
//! Selve ONNX-kjøringen ligger bak featuren `onnx` (se `onnx`-modulen).

use crate::error::ModelError;
use crate::loader::VerifiedModel;
use std::sync::Mutex;

/// Flat f32-tensor i rad-major rekkefølge.
#[derive(Clone, Debug, PartialEq)]
pub struct Tensor {
    pub shape: Vec<usize>,
    pub data: Vec<f32>,
}

impl Tensor {
    pub fn new(shape: Vec<usize>, data: Vec<f32>) -> Result<Tensor, ModelError> {
        let n: usize = shape.iter().product();
        if n != data.len() {
            return Err(ModelError::BadOutputShape {
                expected: shape,
                actual: vec![data.len()],
            });
        }
        Ok(Tensor { shape, data })
    }
}

/// En åpen modell. Én inn-tensor, én ut-tensor (modellen pakkes slik at utdata følger kontrakten i `adapters`).
pub trait OnnxSession: Send + Sync {
    fn run(&self, input: &Tensor) -> Result<Tensor, ModelError>;
}

/// Åpner økter. Implementeres av runtime-integrasjonen; får bare verifiserte modeller.
pub trait SessionFactory: Send + Sync {
    fn open(&self, model: &VerifiedModel) -> Result<Box<dyn OnnxSession>, ModelError>;
}

/// Fabrikk uten runtime: svarer alltid `RuntimeUnavailable`. Standard når featuren `onnx` er av.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoRuntime;

impl SessionFactory for NoRuntime {
    fn open(&self, _model: &VerifiedModel) -> Result<Box<dyn OnnxSession>, ModelError> {
        Err(ModelError::RuntimeUnavailable)
    }
}

/// Mock: returnerer en fast tensor (eller feil) og husker siste inndata.
pub struct MockSession {
    result: Result<Tensor, ModelError>,
    last_input: Mutex<Option<Tensor>>,
}

impl MockSession {
    pub fn returning(output: Tensor) -> MockSession {
        MockSession {
            result: Ok(output),
            last_input: Mutex::new(None),
        }
    }
    pub fn failing(err: ModelError) -> MockSession {
        MockSession {
            result: Err(err),
            last_input: Mutex::new(None),
        }
    }
    pub fn last_input(&self) -> Option<Tensor> {
        self.last_input.lock().ok().and_then(|g| g.clone())
    }
}

impl OnnxSession for MockSession {
    fn run(&self, input: &Tensor) -> Result<Tensor, ModelError> {
        if let Ok(mut g) = self.last_input.lock() {
            *g = Some(input.clone());
        }
        self.result.clone()
    }
}

impl<T: OnnxSession + ?Sized> OnnxSession for std::sync::Arc<T> {
    fn run(&self, input: &Tensor) -> Result<Tensor, ModelError> {
        (**self).run(input)
    }
}

/// Fabrikk som gir ut en delt `MockSession` (for tester).
pub struct MockFactory(pub std::sync::Arc<MockSession>);

impl SessionFactory for MockFactory {
    fn open(&self, _model: &VerifiedModel) -> Result<Box<dyn OnnxSession>, ModelError> {
        Ok(Box::new(self.0.clone()))
    }
}

#[cfg(feature = "onnx")]
pub mod onnx {
    //! Plassholder for den ekte ONNX-fabrikken. Ingen runtime-krate (ort/tract) er lagt inn:
    //! den finnes ikke i Cargo.lock, og nettverket er ikke tilgjengelig for å verifisere den mot
    //! `cargo deny`/`--locked`. Se docs/v1/MODELLER.md «Hva mangler». Fabrikken feiler lukket.
    use super::*;

    #[derive(Clone, Copy, Debug, Default)]
    pub struct OnnxFactory;

    impl SessionFactory for OnnxFactory {
        fn open(&self, _model: &VerifiedModel) -> Result<Box<dyn OnnxSession>, ModelError> {
            Err(ModelError::RuntimeUnavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tensor_validates_length() {
        assert!(Tensor::new(vec![1, 3], vec![0.0; 3]).is_ok());
        assert!(Tensor::new(vec![1, 3], vec![0.0; 2]).is_err());
    }

    #[test]
    fn mock_records_input() {
        let s = MockSession::returning(Tensor::new(vec![1, 1], vec![1.0]).unwrap());
        let i = Tensor::new(vec![1], vec![2.0]).unwrap();
        assert!(s.run(&i).is_ok());
        assert_eq!(s.last_input(), Some(i));
    }
}
