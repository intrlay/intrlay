export { Storage } from "./storage.js";
export { Cursor, count } from "./cursor.js";
export { encodeString, readString, utf8Length } from "./strings.js";
export { EncodingBuffer, prepareWrite } from "./encodingBuffer.js";

/** Distinguishes an unevaluated child view from a cached absent value. */
export const uninitialized: unique symbol = /* @__PURE__ */ Symbol("Intrlay.uninitialized");

/** Presence wrapper for an optional whose payload can itself be null. */
export interface Some<T> { readonly value: T; }
