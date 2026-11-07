import { WrappedScalarsView, ProjectionSetView } from "../../target/compositions/bindings.js";

declare const wrapped: WrappedScalarsView;
wrapped.present = 1;
wrapped.setWriteOnly(2);
// @ts-expect-error generate_setters = false omits the optional mutator
wrapped.readOnly = 3;
// @ts-expect-error generate_getters = false omits the scalar getter
wrapped.writeOnly;
declare const projection: ProjectionSetView;
// Numeric storage keeps its existing mutation API.
projection.readOnly![0] = 1;
