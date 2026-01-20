"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.uniffiCreateRecord = void 0;
/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/
 */
/**
 * @param defaults function that returns the defaults of the record. This is done as a function rather than a literal so
 * that the defaults are calculated lazily, i.e. after everything has been declared.
 * @returns a function to create a new {T} with a partial that requires at least the missing keys to be present.
 */
const uniffiCreateRecord = (defaults) => {
    return (partial) => Object.freeze({ ...defaults(), ...partial });
};
exports.uniffiCreateRecord = uniffiCreateRecord;
