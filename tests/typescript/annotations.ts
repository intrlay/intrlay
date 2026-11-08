import { ContractsView, ProjectedAccessView } from "../../target/regressions/contracts";
import type { MessageInput } from "../../target/conformance/bindings";
import { zeroed } from "../../target/node_modules/@intrlay/runtime/dist/index.js";
import { ReadOnlyEventKind, ReadOnlyStateInput } from "../../target/regressions/input_emission";
import type { ReadOnlyEventView, ReadOnlyStateView, ZReadOnlyEventView } from "../../target/regressions/input_emission";
// @ts-expect-error No generated writer accepts the receive-only record input.
import type { ReadOnlyInput } from "../../target/regressions/input_emission";
// @ts-expect-error A tag enum remains available without a payload input union.
import type { ReadOnlyEventInput } from "../../target/regressions/input_emission";
// @ts-expect-error Export aliases follow the same input reachability rule.
import type { ZReadOnlyEventInput } from "../../target/regressions/input_emission";

type SameType<T, U> = (<V>() => V extends T ? 1 : 2) extends (<V>() => V extends U ? 1 : 2) ? true : false;
const unionTag: SameType<ReadOnlyEventView["tag"], ReadOnlyEventKind> = true;
const aliasTag: SameType<ZReadOnlyEventView["tag"], ReadOnlyEventKind> = true;
const valueTag: SameType<ReadOnlyStateView["tag"], ReadOnlyStateInput> = true;

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
