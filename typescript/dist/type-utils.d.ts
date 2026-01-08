export type StructuralEquality<T, U> = [T] extends [U] ? [U] extends [T] ? true : false : false;
