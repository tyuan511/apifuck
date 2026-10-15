import * as YAML from 'yaml';
import {parseBruRequest,stringifyBruRequest,parseBruEnvironment,stringifyBruEnvironment,parseBruCollection,stringifyBruCollection} from '@bruno-source/bruno-filestore/src/formats/bru/index.ts';
import parseItem from '@bruno-source/bruno-filestore/src/formats/yml/parseItem.ts';
import stringifyItem from '@bruno-source/bruno-filestore/src/formats/yml/stringifyItem.ts';
import parseEnvironment from '@bruno-source/bruno-filestore/src/formats/yml/parseEnvironment.ts';
import stringifyEnvironment from '@bruno-source/bruno-filestore/src/formats/yml/stringifyEnvironment.ts';
import parseFolder from '@bruno-source/bruno-filestore/src/formats/yml/parseFolder.ts';
import stringifyFolder from '@bruno-source/bruno-filestore/src/formats/yml/stringifyFolder.ts';
import {expect,assert} from 'chai';
globalThis.expect=expect;globalThis.assert=assert;
globalThis.convertBru=(kind,source,direction)=>{
  globalThis.bruResult=null;
  try {
    let result;
    if(direction==='import') {
      const parsed=kind==='environment'?parseBruEnvironment(source):kind==='folder'?parseBruCollection(source):parseBruRequest(source);
      result=Promise.resolve(parsed).then(value=>{if(kind==='environment')return stringifyEnvironment(value);if(kind==='folder')return stringifyFolder(value);const document=YAML.parse(stringifyItem(value));if(document.info)document.info.seq=Number.isFinite(value.seq)?value.seq:1;return YAML.stringify(document);});
    }else{
      const parsed=kind==='environment'?parseEnvironment(source):kind==='folder'?parseFolder(source):parseItem(source);
      result=Promise.resolve(parsed).then(value=>kind==='environment'?stringifyBruEnvironment(value):kind==='folder'?stringifyBruCollection(value,true):stringifyBruRequest(value));
    }
    Promise.resolve(result).then(value=>globalThis.bruResult=JSON.stringify({value}),error=>globalThis.bruResult=JSON.stringify({error:String(error)}));
  }catch(error){globalThis.bruResult=JSON.stringify({error:String(error)});}
};
