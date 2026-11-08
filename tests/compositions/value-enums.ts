import { SparseStateInput, SparseStateView, StateAliasInput, StateAliasView, ValueChoiceAliasInput, ValueChoiceAliasKind, ValueChoiceAliasView, ValueChoiceKind, ValueEnumsInput, ValueEnumsView } from "../../target/compositions/value-enums.js";

export function checkValueEnums(expected: Uint8Array): void {
    const value: ValueEnumsInput = {
        choice: {
            kind: ValueChoiceKind.Present,
            state: SparseStateInput.Last,
            optional: SparseStateInput.Middle,
            fixed: [SparseStateInput.First, SparseStateInput.Last],
            states: [SparseStateInput.Middle, SparseStateInput.First],
            text: "hello",
        },
    };
    const view = ValueEnumsView.create(value);
    if (value.choice.kind === ValueChoiceKind.Present && value.choice.state !== SparseStateInput.Last) {
        throw new Error("enum kind failed to narrow its payload");
    }
    const alias: ValueChoiceAliasInput = { kind: ValueChoiceAliasKind.Empty };
    if (ValueChoiceAliasView.create(alias).tag !== 9 || view.choice.tag !== 91) {
        throw new Error("payload enum kinds lost sparse tags or aliases");
    }
    // @ts-expect-error Payload enum inputs require a numeric enum discriminant.
    const invalid: ValueChoiceAliasInput = { kind: "Empty" };
    void invalid;
    if (view.bytes.length !== expected.length || view.bytes.some((byte, index) => byte !== expected[index])) {
        throw new Error("native enum bytes differ from Rust");
    }
    if (view.choice.presentState?.tag !== 0xffffffff || view.choice.presentOptional?.tag !== 37 || view.choice.presentFixed?.get(1).tag !== 0xffffffff || view.choice.presentStates?.get(1).tag !== 3) {
        throw new Error("nested native enum fields lost their sparse tags");
    }
    const state = SparseStateView.create(SparseStateInput.Last);
    state.replace(SparseStateInput.Middle);
    if (state.tag !== 37 || StateAliasView.create(StateAliasInput.First).tag !== 3) {
        throw new Error("native enum replacement or export alias failed");
    }
    if (SparseStateView.create(37).tag !== 37 || ValueChoiceAliasView.create({ kind: 9 }).tag !== 9) {
        throw new Error("enum member constants must match their numeric literal values");
    }
    for (const createInvalid of [
        () => SparseStateView.create(0 as SparseStateInput),
        () => ValueChoiceAliasView.create({ kind: 0 } as unknown as ValueChoiceAliasInput),
    ]) {
        let rejected = false;
        try { createInvalid(); } catch (error) { rejected = error instanceof RangeError; }
        if (!rejected) throw new Error("unknown enum tag was accepted");
    }
}
