declare const sequenceBrand: unique symbol;

/** An explicit request for this many zero-initialized sequence elements. */
export type ZeroedSequence = number & { readonly [sequenceBrand]: true };

const fixedArray: unique symbol = /* @__PURE__ */ Symbol("Intrlay.zeroed");

/** Requests zero initialization using the fixed array's schema length. */
export type Zeroed = typeof fixedArray;

/** Zero-initialize a fixed array using its schema length. */
export function zeroed(): Zeroed;
/** Zero-initialize a sequence without allocating an intermediate element array. */
export function zeroed(length: number): ZeroedSequence;
export function zeroed(length?: number): Zeroed | ZeroedSequence {
    return length === undefined ? fixedArray : length as ZeroedSequence;
}
