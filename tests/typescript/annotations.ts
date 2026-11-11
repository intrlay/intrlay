import { ContractsView, ProjectedAccessView, NumericTag, ZNumericTag, NumericEnumFieldsView } from "../../target/regressions/contracts";
import type { NumericInputsInput, IndexedEnumsView, IndexedChoiceKind } from "../../target/regressions/contracts";
import type { MessageInput } from "../../target/conformance/bindings";
import { zeroed } from "../../target/node_modules/@intrlay/runtime/dist/index.js";
import { ReadOnlyEventKind, ReadOnlyState } from "../../target/regressions/input_emission";
import type { ReadOnlyEventView, ReadOnlyStateView, ZReadOnlyEventView } from "../../target/regressions/input_emission";
// @ts-expect-error No generated writer accepts the receive-only record input.
import type { ReadOnlyInput } from "../../target/regressions/input_emission";
// @ts-expect-error A tag enum remains available without a payload input union.
import type { ReadOnlyEventInput } from "../../target/regressions/input_emission";
// @ts-expect-error Export aliases follow the same input reachability rule.
import type { ZReadOnlyEventInput } from "../../target/regressions/input_emission";

type SameType<T, U> = (<V>() => V extends T ? 1 : 2) extends (<V>() => V extends U ? 1 : 2) ? true : false;
const numericEnumFields = NumericEnumFieldsView.create({ plain: ZNumericTag.First, numeric: new Uint32Array([NumericTag.Second]) });
const originalEnum: NumericTag = numericEnumFields.plain;
void originalEnum;
const numericInput: NumericInputsInput = { matrix: new Float32Array(4), points: new Float32Array(6), tags: new Uint32Array(2), empty: new Float32Array(0) };
// @ts-expect-error Explicit numeric projection takes packed lanes, not nested objects.
const nestedNumericInput: NumericInputsInput["points"] = [[1, 2, 3]];
declare const indexedEnums: IndexedEnumsView;
const indexedTag: IndexedChoiceKind = indexedEnums.items.getTag(0);
const indexedOptional: number | null | undefined = indexedEnums.items.getOptionalValue(0);
// @ts-expect-error Only the fixed array requests element views.
indexedEnums.items.get(0);
indexedEnums.fixed.get(0);
void [numericInput, nestedNumericInput, indexedTag, indexedOptional];
const unionTag: SameType<ReadOnlyEventView["tag"], ReadOnlyEventKind> = true;
const aliasTag: SameType<ZReadOnlyEventView["tag"], ReadOnlyEventKind> = true;
const valueTag: SameType<ReadOnlyStateView["tag"], ReadOnlyState> = true;

const zeroedPoints: MessageInput["points"] = zeroed(3);
const zeroedFixed: MessageInput["fixed"] = zeroed();
// @ts-expect-error A bare number is not a sequence input.
const numericPoints: MessageInput["points"] = 3;
// @ts-expect-error Fixed arrays take the length from their schema.
const sizedFixed: MessageInput["fixed"] = zeroed(3);
// @ts-expect-error Variable sequences need an explicit length.
const unsizedPoints: MessageInput["points"] = zeroed();
// @ts-expect-error Enum payloads cannot be initialized by zeroing their bytes.
const zeroedCommands: MessageInput["commands"] = zeroed(3);
declare const view: ContractsView;
const number: number = view.identifier;
view.identifier = number;
view.editable.value = number;
view.writeOnly.value = number;
view.readOnly.value = 1;
view.readOnlyArray[0] = 1;
view.readOnlyArray.fill(1);
view.readOnlyArray.subarray()[0] = 1;
const hidden = view.writeOnly.value;
view.readOnlyNested!.get(0).value = 1;
// @ts-expect-error The child declaration does not generate a setter.
view.fixed.value = 1;
declare const projected: ProjectedAccessView;
projected.value[0] = 1;
const structured: number = projected.structured.branch.leaf.value;
projected.indexed.setValue(0, structured);
const indexed: number = projected.indexed.getValue(0);
projected.ordinary.get(0).value = indexed;
projected.indexed.get(0).value = indexed;
projected.direct.setValue(0, indexed);
// Shared collections expose the union of their users' requested operations.
projected.ordinary.getValue(0);
projected.ordinary.setValue(0, 1);
projected.direct.get(0);

import type { FlattenedAccessView, IndexedOnlyView, IndexedFixedView, IndexedWriteOnlyView } from "../../target/regressions/contracts";
// @ts-expect-error No reachable accessor returns this element view.
import type { IndexedEntryView } from "../../target/regressions/contracts";

declare const indexedOnly: IndexedOnlyView;
indexedOnly.items.setId(0, indexedOnly.items.getId(0));
indexedOnly.items.setLanesComponent(0, 1, 2);
indexedOnly.items.setDirectComponent(0, 2, 3);
// @ts-expect-error indexed_only omits the whole-element getter.
indexedOnly.items.get(0);
// @ts-expect-error Automatic numeric arrays do not imply component helpers.
indexedOnly.items.getPlainComponent(0, 0);
// @ts-expect-error Explicit numeric projection does not imply component helpers either.
indexedOnly.items.getProjectedComponent(0, 0);

declare const indexedFixed: IndexedFixedView;
indexedFixed.withComponents.setChild(0, { value: 31 });
indexedFixed.withComponents.setLanesComponent(0, 1, 20);
// @ts-expect-error A field's explicit false overrides the collection default.
indexedFixed.withComponents.getPlainComponent(0, 0);
// @ts-expect-error Options on one collection field do not affect another field of the same type.
indexedFixed.withoutComponents.getLanesComponent(0, 0);
indexedFixed.withComponents.getChild(0).value = 31;

declare const indexedWriteOnly: IndexedWriteOnlyView;
indexedWriteOnly.items.getValues(0).set(1, 20);
indexedWriteOnly.fixed.getValues(0).set(0, 30);
// @ts-expect-error Mutation navigation does not restore the whole-element getter.
indexedWriteOnly.items.get(0);
// @ts-expect-error Scalar reads remain omitted.
indexedWriteOnly.items.getId(0);
declare const flattened: FlattenedAccessView;
flattened.min0 = 1;
flattened.detailsEnabled = true;
flattened.maybe = null;
// @ts-expect-error Flattened leaves retain their distinct scalar types.
flattened.detailsWeight = false;
// @ts-expect-error Child setter generation is respected through flattening.
flattened.detailsFixed = 1;
// @ts-expect-error Flattened fields have no intermediate object accessor.
flattened.max.x = 1;
