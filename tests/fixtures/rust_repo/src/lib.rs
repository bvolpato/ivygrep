pub fn calculate_tax(amount: f64, rate: f64) -> f64 {
    amount * rate
}

pub fn subtotal(items: &[f64]) -> f64 {
    items.iter().sum()
}

pub fn invoice_total(items: &[f64], rate: f64) -> f64 {
    let base = subtotal(items);
    base + calculate_tax(base, rate)
}
