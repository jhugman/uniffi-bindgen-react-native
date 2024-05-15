/**
 * @param defaults function that returns the defaults of the record. This is done as a function rather than a literal so
 * that the defaults are calculated lazily, i.e. after everything has been declared.
 * @returns a function to create a new {T} with a partial that requires at least the missing keys to be present.
 */
export const uniffiCreateRecord = <T, D extends Partial<T>>(
  defaults: () => D,
) => {
  type MissingKeys = Omit<T, keyof D>;
  return (partial: Required<MissingKeys> & Partial<D>): T =>
    Object.freeze({ ...defaults(), ...partial } as T);
};
