export function calculateTax(amount: number, rate: number): number {
  return amount * rate;
}

export function calculateTotal(values: number[], rate: number): number {
  const subtotal = values.reduce((sum, value) => sum + value, 0);
  return subtotal + calculateTax(subtotal, rate);
}
