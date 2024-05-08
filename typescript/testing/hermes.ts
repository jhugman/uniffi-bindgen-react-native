declare function print(...args: any): void;

class Console {
  log(...args: any): void {
    print(...args);
  }

  error(...args: any): void {
    print(...args);
  }

  warn(...args: any): void {
    print(...args);
  }

  debug(...args: any): void {
    print(...args.map(stringify));
  }
}

export const console = new Console();

export function stringify(a: any): string {
  return JSON.stringify(a, replacer);
}

function replacer(key: string, value: any): any {
  if (value instanceof Set) {
    return [...value];
  }
  if (value instanceof Map) {
    return Object.fromEntries(value);
  }
  if (typeof value === "bigint") {
    return `BigInt("${value}")`;
  }
  if (value.constructor !== Object && typeof value.toString === "function") {
    return value.toString();
  }
  if (typeof value.asJSON === "function") {
    return replacer(key, value.asJSON());
  }

  return value;
}
