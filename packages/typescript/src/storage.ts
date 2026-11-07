if (new Uint8Array(new Uint32Array([1]).buffer)[0] !== 1) {
    throw new Error("Intrlay requires little-endian TypedArray storage");
}

export class Storage {
    public readonly bytes: Uint8Array;
    private _i8?: Int8Array;
    private _i16?: Int16Array;
    private _u16?: Uint16Array;
    private _i32?: Int32Array;
    private _u32?: Uint32Array;
    private _i64?: BigInt64Array;
    private _u64?: BigUint64Array;
    private _f32?: Float32Array;
    private _f64?: Float64Array;

    public constructor(bytes: Uint8Array) {
        this.bytes = bytes;
    }

    public get u8(): Uint8Array { return this.bytes; }

    public get i8(): Int8Array {
        return this._i8 ??= new Int8Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            this.bytes.byteLength
        );
    }

    public get i16(): Int16Array {
        return this._i16 ??= new Int16Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 2)
        );
    }

    public get u16(): Uint16Array {
        return this._u16 ??= new Uint16Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 2)
        );
    }

    public get i32(): Int32Array {
        return this._i32 ??= new Int32Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 4)
        );
    }

    public get u32(): Uint32Array {
        return this._u32 ??= new Uint32Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 4)
        );
    }

    public get i64(): BigInt64Array {
        return this._i64 ??= new BigInt64Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 8)
        );
    }

    public get u64(): BigUint64Array {
        return this._u64 ??= new BigUint64Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 8)
        );
    }

    public get f32(): Float32Array {
        return this._f32 ??= new Float32Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 4)
        );
    }

    public get f64(): Float64Array {
        return this._f64 ??= new Float64Array(
            this.bytes.buffer,
            this.bytes.byteOffset,
            Math.floor(this.bytes.byteLength / 8)
        );
    }

}
