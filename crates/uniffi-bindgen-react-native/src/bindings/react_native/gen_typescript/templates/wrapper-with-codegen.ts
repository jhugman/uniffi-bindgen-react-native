import type {TurboModule} from 'react-native/Libraries/TurboModule/RCTExport';

import {TurboModuleRegistry} from 'react-native';

export interface Spec extends TurboModule {

{%- for func in ci.iter_ffi_function_definitions() %}
  readonly {{ func.name() }}: (
    {%- call ts::arg_list_ffi_decl(func) %}) =>
    {%- match func.return_type() %}{% when Some with (return_type) %} {{ return_type.borrow()|ffi_type_name_by_value }}{% when None %} void{% endmatch %};
{%- endfor %}
}

export default (TurboModuleRegistry.getEnforcing<Spec>("{{ config.cpp_module }}"));
{% import "macros.ts" as ts %}
