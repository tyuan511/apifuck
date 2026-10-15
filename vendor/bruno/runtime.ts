import chai from 'chai';
import jsonQuery from 'json-query';
globalThis.expect=chai.expect;globalThis.assert=chai.assert;globalThis.jsonQuery=(path,data)=>jsonQuery(path,{data}).value;
import Ajv from 'ajv';
import addFormats from 'ajv-formats';
chai.use((chai)=>{
  chai.Assertion.addMethod('jsonSchema',function(schema,options){
    const ajv=new Ajv({allErrors:true,...options});addFormats(ajv);
    const validate=ajv.compile(schema);const valid=validate(this._obj);
    this.assert(valid,`JSON Schema validation failed: ${ajv.errorsText(validate.errors)}`,'Expected value not to match JSON Schema');
  });
  chai.Assertion.addProperty('json',function(){const value=this._obj;this.assert(value!==null&&typeof value==='object','Expected JSON object or array','Expected non-JSON value');});
});
