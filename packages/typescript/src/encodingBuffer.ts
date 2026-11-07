import { Cursor } from "./cursor.js";
import { Storage } from "./storage.js";

/** Reuses the cursor and numeric views when repeatedly encoding into one buffer.
 * Previous views still reference this storage and observe subsequent writes.
 */
export class EncodingBuffer {
    public readonly bytes: Uint8Array;
    public readonly cursor = new Cursor(0);
    private _storage: Storage | undefined;

    public constructor(bytes: Uint8Array) {
        this.bytes = bytes;
    }

    public prepare(size: number, alignment: number): Storage {
        checkCapacity(this.bytes, size, alignment);
        const storage = this._storage?.bytes.byteLength === size
            ? this._storage
            : (this._storage = new Storage(this.bytes.subarray(0, size)));
        storage.bytes.fill(0);
        return storage;
    }
}

/** Borrows and initializes exactly the part occupied by the new encoding. */
export function prepareWrite(bytes: Uint8Array | EncodingBuffer, size: number, alignment: number): Storage {
    if (ArrayBuffer.isView(bytes)) {
        checkCapacity(bytes, size, alignment);
        const storage = new Storage(bytes.subarray(0, size));
        storage.bytes.fill(0);
        return storage;
    } else {
        return bytes.prepare(size, alignment);
    }
}

function checkCapacity(bytes: Uint8Array, size: number, alignment: number): void {
    if (bytes.byteLength < size || bytes.byteOffset % alignment !== 0) {
        throw new RangeError("buffer capacity or alignment");
    }
}
