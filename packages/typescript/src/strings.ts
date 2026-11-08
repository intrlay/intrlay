import type { Storage } from "./storage.js";

let encoder: TextEncoder | undefined;
let decoder: TextDecoder | undefined;

/** Writes UTF-8 into an already sized destination. */
export function encodeString(value: string, target: Uint8Array): void {
    (encoder ??= new TextEncoder()).encodeInto(value, target);
}

/** Decodes a borrowed string; a leading U+FEFF belongs to the value. */
export function readString(storage: Storage, offset: number): string {
    const integers = storage.u32;
    const start = offset + integers[offset >>> 2]!;
    const length = integers[(offset >>> 2) + 1]!;
    return (decoder ??= new TextDecoder("utf-8", { ignoreBOM: true }))
        .decode(storage.bytes.subarray(start, start + length));
}

export function utf8Length(value: string): number {
    let length = 0;
    for (let index = 0; index < value.length; ++index) {
        const codeUnit = value.charCodeAt(index);
        if (codeUnit < 0x80) {
            ++length;
        } else if (codeUnit < 0x800) {
            length += 2;
        } else if (
            codeUnit >= 0xd800 && codeUnit <= 0xdbff &&
            index + 1 < value.length &&
            value.charCodeAt(index + 1) >= 0xdc00 && value.charCodeAt(index + 1) <= 0xdfff
        ) {
            length += 4;
            ++index;
        } else {
            length += 3;
        }
    }
    return length;
}

