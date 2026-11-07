import { ContractsView, ProjectedAccessView } from "../../target/regressions/contracts";
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
