/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
/**
 * This is used in both the generated code and the test.
 * To get it into the generated typescript, it should be part of a
 * custom_type in the {@link ../uniffi.toml | uniffi.toml file}.
 */
export class Url {
  constructor(private urlString: string) {}
  toString(): string {
    return this.urlString;
  }
}

export function dateToSeconds(date: Date): number {
  return date.getTime() / 1000.0;
}

export function secondsToDate(seconds: number): Date {
  return new Date(seconds * 1000);
}
