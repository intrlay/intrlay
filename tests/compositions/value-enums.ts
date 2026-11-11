import { SparseState, SparseStateView, StateAlias, StateAliasView, ValueChoiceAliasInput, ValueChoiceAliasKind, ValueChoiceAliasView, ValueChoiceKind, ValueEnumsInput, ValueEnumsView } from "../../target/compositions/value-enums.js";
import { ProjectedEnumsInput, ProjectedEnumsView, ProjectedMode } from "../../target/compositions/value-enums.js";
import { NestedEnumValuesInput, NestedEnumValuesView, PaddedState } from "../../target/compositions/value-enums.js";
import * as enumBindings from "../../target/compositions/value-enums.js";

export function checkValueEnums(expected: Uint8Array): void {
    const value: ValueEnumsInput = {
        choice: {
            kind: ValueChoiceKind.Present,
            state: SparseState.Last,
            optional: SparseState.Middle,
            fixed: [SparseState.First, SparseState.Last],
            states: [SparseState.Middle, SparseState.First],
            text: "hello",
        },
    };
    const view = ValueEnumsView.create(value);
    if (value.choice.kind === ValueChoiceKind.Present && value.choice.state !== SparseState.Last) {
        throw new Error("enum kind failed to narrow its payload");
    }
    const alias: ValueChoiceAliasInput = { kind: ValueChoiceAliasKind.Empty };
    if (ValueChoiceAliasView.create(alias).tag !== 9 || view.choice.tag !== 91) {
        throw new Error("payload enum kinds lost sparse tags or aliases");
    }
    // @ts-expect-error Payload enum inputs require a numeric enum discriminant.
    const invalid: ValueChoiceAliasInput = { kind: "Empty" };
    void invalid;
    if (view.storage.bytes.length !== expected.length || view.storage.bytes.some((byte, index) => byte !== expected[index])) {
        throw new Error("native enum bytes differ from Rust");
    }
    if (view.choice.presentState !== 0xffffffff || view.choice.presentOptional !== 37 || view.choice.presentFixed?.get(1) !== 0xffffffff || view.choice.presentStates?.get(1) !== 3) {
        throw new Error("nested native enum fields lost their sparse tags");
    }
    const state = SparseStateView.create(SparseState.Last);
    state.replace(SparseState.Middle);
    if (state.tag !== 37 || StateAliasView.create(StateAlias.First).tag !== 3) {
        throw new Error("native enum replacement or export alias failed");
    }
    if (SparseStateView.create(37).tag !== 37 || ValueChoiceAliasView.create({ kind: 9 }).tag !== 9) {
        throw new Error("enum member constants must match their numeric literal values");
    }
    for (const createInvalid of [
        () => SparseStateView.create(0 as SparseState),
        () => ValueChoiceAliasView.create({ kind: 0 } as unknown as ValueChoiceAliasInput),
    ]) {
        let rejected = false;
        try { createInvalid(); } catch (error) { rejected = error instanceof RangeError; }
        if (!rejected) throw new Error("unknown enum tag was accepted");
    }
}

export function checkEnumProjection(expected: Uint8Array, updated: Uint8Array): void {
    const value: ProjectedEnumsInput = {
        states: new Uint32Array([SparseState.Middle, SparseState.First, SparseState.Last]),
        fixed: new Uint32Array([SparseState.First, SparseState.Last]),
        empty: new Uint32Array(),
        current: new Uint32Array([SparseState.Last]),
        mode: new Uint32Array([ProjectedMode.Idle]),
        modes: [new Uint32Array([ProjectedMode.Idle]), new Uint32Array([ProjectedMode.Active])],
    };
    const owner = new Uint8Array(expected.length + 32).fill(0xa5);
    const view = ProjectedEnumsView.createInPlace(owner.subarray(16), value);
    if (view.storage.bytes.length !== expected.length || view.storage.bytes.some((byte, index) => byte !== expected[index])) {
        throw new Error("projected enum bytes differ from Rust");
    }
    for (const region of [view.states, view.fixed, view.empty, view.current, view.mode, view.modes.get(1)]) {
        if (!(region instanceof Uint32Array) || region.buffer !== owner.buffer) {
            throw new Error("enum projection must borrow unsigned tag storage");
        }
    }
    if (view.states[2] !== 0xffffffff || view.fixed[0] !== 3 || view.empty.length !== 0 ||
        view.current[0] !== 0xffffffff || view.mode?.[0] !== 3 || view.modes.get(1)[0] !== 255) {
        throw new Error("enum projection lost tags or used the source enum's native width");
    }
    view.states[1] = SparseState.Last;
    view.fixed[0] = SparseState.Middle;
    view.current[0] = SparseState.First;
    view.mode![0] = ProjectedMode.Active;
    view.modes.get(1)[0] = ProjectedMode.Idle;
    if (view.storage.bytes.length !== updated.length || view.storage.bytes.some((byte, index) => byte !== updated[index]) ||
        owner.subarray(0, 16).some(byte => byte !== 0xa5) ||
        owner.subarray(16 + expected.length).some(byte => byte !== 0xa5)) {
        throw new Error("enum projection writes must update only the selected tags");
    }
    const reopened = new ProjectedEnumsView(view.storage.bytes);
    if (reopened.states[1] !== 0xffffffff || reopened.fixed[0] !== 37 || reopened.mode?.[0] !== 255) {
        throw new Error("enum projection updates were not visible to another view");
    }
}

export function checkNestedEnums(expected: Uint8Array): void {
    const input: NestedEnumValuesInput = {
        state: PaddedState.Last, optional: PaddedState.Middle, boxed: PaddedState.First,
        fixed: [PaddedState.First, PaddedState.Last],
        states: [PaddedState.Middle, PaddedState.First],
        indexed: [{ state: PaddedState.Middle, optional: PaddedState.First }],
        projected: new Uint32Array([PaddedState.First, 0, 0, 0, PaddedState.Last, 0, 0, 0]),
        writeOnly: PaddedState.First, readOnly: PaddedState.Middle,
    };
    const view = NestedEnumValuesView.create(input);
    const state: PaddedState = view.state;
    if (state !== PaddedState.Last || view.optional !== PaddedState.Middle || view.boxed !== PaddedState.First ||
        view.fixed.get(1) !== PaddedState.Last || view.states.get(0) !== PaddedState.Middle ||
        view.indexed.getState(0) !== PaddedState.Middle || view.indexed.getOptional(0) !== PaddedState.First) {
        throw new Error("nested accessors must return enum values with the encoded stride");
    }
    if (view.storage.bytes.length !== expected.length || view.storage.bytes.some((byte, index) => byte !== expected[index]) ||
        "PaddedStateView" in enumBindings || "writeOnly" in view || "setReadOnly" in view) {
        throw new Error("nested enum encoding or generated API differs from its declaration");
    }
    if (view.projected.length !== 8 || view.projected[0] !== 3 || view.projected[4] !== 0xffffffff ||
        view.projected.some((value, index) => index % 4 !== 0 && value !== 0)) {
        throw new Error("padded enum projection must include all u32 lanes");
    }
    view.state = PaddedState.First;
    view.optional = null;
    if (view.optional !== null) throw new Error("optional enum must become absent");
    view.optional = PaddedState.Last;
    view.setBoxed(PaddedState.Middle);
    view.fixed.set(1, PaddedState.Middle);
    view.states.set(0, PaddedState.Last);
    view.indexed.setState(0, PaddedState.Last);
    view.indexed.setOptional(0, null);
    if (view.indexed.getOptional(0) !== null) throw new Error("indexed optional enum must become absent");
    view.indexed.setOptional(0, PaddedState.Middle);
    view.setWriteOnly(PaddedState.Last);
    view.projected[4] = PaddedState.Middle;
    const reopened = new NestedEnumValuesView(view.storage.bytes);
    if (reopened.state !== PaddedState.First || reopened.optional !== PaddedState.Last ||
        reopened.boxed !== PaddedState.Middle || reopened.fixed.get(1) !== PaddedState.Middle ||
        reopened.states.get(0) !== PaddedState.Last || reopened.indexed.getState(0) !== PaddedState.Last ||
        reopened.indexed.getOptional(0) !== PaddedState.Middle || reopened.readOnly !== PaddedState.Middle ||
        reopened.projected[4] !== PaddedState.Middle) {
        throw new Error("enum mutations must remain visible through fresh views");
    }
}
