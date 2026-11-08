import { readFileSync } from "node:fs";
import { checkValueEnums } from "../../target/compositions/compiled/tests/compositions/value-enums.js";

checkValueEnums(readFileSync(new URL("../../target/compositions/value-enums.bin", import.meta.url)));
console.log("TypeScript native enum inputs passed");
