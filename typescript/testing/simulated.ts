/**
 * This function simulates the Rust toString method, which expands the exponent,
 * so 1.7e6 is presented as 1700000, and 3.14e-15 is presented as 0.00000000000000314.
 * @param n
 * @returns
 */
export function numberToString(n: number): string {
  const [base, exponent] = n.toExponential().split("e");
  const [whole, fractional = ""] = base.split(".");
  const e = parseInt(exponent, 10);
  const result: string[] = [];
  if (e > 0) {
    let padding = e - fractional.length;
    if (padding > 0) {
      result.push(whole, fractional, "0".repeat(padding));
    } else if (padding < 0) {
      result.push(
        whole,
        fractional.substring(0, e),
        ".",
        fractional.substring(e),
      );
    } else {
      result.push(whole, fractional);
    }
  } else if (e < 0) {
    result.push("0.", "0".repeat(-e - 1), whole, fractional);
  } else if (fractional.length > 0) {
    result.push(whole, ".", fractional);
  } else {
    result.push(whole);
  }
  return result.join("");
}
