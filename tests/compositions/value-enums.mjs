import { readFileSync } from "node:fs";
import { checkEnumProjection, checkNestedEnums, checkValueEnums } from "../../target/compositions/compiled/tests/compositions/value-enums.js";

checkValueEnums(readFileSync(new URL("../../target/compositions/value-enums.bin", import.meta.url)));
checkEnumProjection(
    readFileSync(new URL("../../target/compositions/projected-enums.bin", import.meta.url)),
    readFileSync(new URL("../../target/compositions/projected-enums-updated.bin", import.meta.url)),
);
checkNestedEnums(readFileSync(new URL("../../target/compositions/nested-enums.bin", import.meta.url)));
console.log("TypeScript native enum inputs passed");
