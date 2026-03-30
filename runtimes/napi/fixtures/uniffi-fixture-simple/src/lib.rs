uniffi::setup_scaffolding!();

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum ArithmeticError {
    #[error("{reason}")]
    DivisionByZero { reason: String },
}

#[uniffi::export]
pub fn add(a: u32, b: u32) -> u32 {
    a + b
}

#[uniffi::export]
pub fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

#[uniffi::export]
pub fn divide(a: f64, b: f64) -> Result<f64, ArithmeticError> {
    if b == 0.0 {
        Err(ArithmeticError::DivisionByZero {
            reason: "cannot divide by zero".to_string(),
        })
    } else {
        Ok(a / b)
    }
}

#[uniffi::export]
pub async fn async_add(a: u32, b: u32) -> u32 {
    a + b
}

#[uniffi::export]
pub async fn async_greet(name: String) -> String {
    format!("Hello, {name}!")
}

#[uniffi::export(callback_interface)]
pub trait Calculator: Send + Sync {
    fn add(&self, a: u32, b: u32) -> u32;
    fn concatenate(&self, a: String, b: String) -> String;
}

#[uniffi::export]
pub fn use_calculator(calc: Box<dyn Calculator>, a: u32, b: u32) -> u32 {
    calc.add(a, b)
}

#[uniffi::export]
pub fn use_calculator_strings(
    calc: Box<dyn Calculator>,
    a: String,
    b: String,
) -> String {
    calc.concatenate(a, b)
}

#[uniffi::export]
pub async fn use_calculator_from_thread(calc: Box<dyn Calculator>, a: u32, b: u32) -> u32 {
    let (tx, rx) = tokio::sync::oneshot::channel();
    std::thread::spawn(move || {
        let result = calc.add(a, b);
        let _ = tx.send(result);
    });
    rx.await.unwrap()
}

#[uniffi::export(with_foreign)]
#[async_trait::async_trait]
pub trait AsyncFetcher: Send + Sync {
    async fn fetch(&self, input: String) -> String;
}

#[uniffi::export]
pub async fn use_async_fetcher(
    fetcher: std::sync::Arc<dyn AsyncFetcher>,
    input: String,
) -> String {
    fetcher.fetch(input).await
}
