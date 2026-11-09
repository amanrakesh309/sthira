import { Node } from '@tiptap/core';
import { entityHref, parseEntityHref } from '../api/connections';
import type { EntityRef } from '../types/sthira';

const uri = String.raw`(?:sthira:\/\/(?:artifact|project)\/|#\/sthira\/(?:artifact|project)\/)[^\s\x60<>\[\]()]+`;
const labelled = new RegExp(String.raw`^([^\n\x60\[\]<>]+?)\s+[–—·-]\s+(\x60?)(${uri})\2`);
const quoted = new RegExp(String.raw`^\x60(${uri})\x60`);
const bare = new RegExp(`^(${uri})`);
const startPattern = new RegExp(String.raw`(?:[^\n\x60\[\]<>]+?\s+[–—·-]\s+)?\x60?${uri}`);
export interface ReferenceToken { raw:string; label:string; href:string; entity:EntityRef }
export function matchInternalReference(source:string):ReferenceToken|null {
  const labelledMatch=source.match(labelled);
  const quotedMatch=source.match(quoted);
  const bareMatch=source.match(bare);
  const candidate=labelledMatch?.[3] || quotedMatch?.[1] || bareMatch?.[1];
  if(!candidate)return null;
  const entity=parseEntityHref(candidate);
  if(!entity)return null;
  return { raw:labelledMatch?.[0] || quotedMatch?.[0] || bareMatch![0], label:labelledMatch?.[1].trim() || (entity.kind==='project'?'Open saved project':'Open saved page'),href:entityHref(entity),entity };
}

// A display node keeps the original Markdown verbatim during surrounding edits.
// This makes old code-wrapped references useful without rewriting notes on open.
export const InternalReference = Node.create({
  name:'internalReference',group:'inline',inline:true,atom:true,
  addAttributes:()=>({raw:{default:''},label:{default:'Open saved page'},href:{default:''}}),
  parseHTML:()=>[{tag:'a[data-workspace-reference]',getAttrs:element=>{
    const el=element as HTMLElement;
    const entity=parseEntityHref(el.getAttribute('href')||undefined);
    const raw=el.getAttribute('data-reference-markdown')||'';
    const token=matchInternalReference(raw);
    if(!entity||!token||token.raw!==raw||token.href!==entityHref(entity))return false;
    return {raw:token.raw,label:token.label,href:token.href};
  }}],
  renderHTML:({node})=>{
    const entity=parseEntityHref(node.attrs.href);
    if(!entity)return ['span',{},node.attrs.label];
    return ['a',{'data-workspace-reference':'','data-reference-markdown':node.attrs.raw,href:entityHref(entity),title:'Preview saved reference',class:'connection-inline-link',contenteditable:'false',tabindex:'0'},node.attrs.label];
  },
  markdownTokenName:'internalReference',
  markdownTokenizer:{name:'internalReference',level:'inline',start:source=>source.search(startPattern),tokenize:source=>{
    const token=matchInternalReference(source);
    return token?{type:'internalReference',raw:token.raw,label:token.label,href:token.href}:undefined;
  }},
  parseMarkdown:(token,helpers)=>helpers.createNode('internalReference',{raw:token.raw,label:token.label,href:token.href}),
  renderMarkdown:node=>node.attrs?.raw||'',
});

type MarkdownNode={type:string;value?:string;url?:string;children?:MarkdownNode[]};
export function remarkInternalReferences() {
  return (tree:MarkdownNode)=>{
    const visit=(node:MarkdownNode)=>{
      if(['code','html','link','image'].includes(node.type)||!node.children)return;
      const children=node.children;
      if(node.type==='paragraph'&&children.length===2&&children[0].type==='text'&&children[1].type==='inlineCode'){
        const token=matchInternalReference(`${children[0].value}\`${children[1].value}\``);
        if(token&&token.raw===`${children[0].value}\`${children[1].value}\``){node.children=[{type:'link',url:token.href,children:[{type:'text',value:token.label}]}];return;}
      }
      node.children=children.flatMap(child=>{
        if(child.type==='inlineCode'){
          const entity=parseEntityHref(child.value);
          return entity?[{type:'link',url:entityHref(entity),children:[{type:'text',value:entity.kind==='project'?'Open saved project':'Open saved page'}]}]:[child];
        }
        if(child.type==='text'&&child.value){
          const output:MarkdownNode[]=[];let rest=child.value;
          while(rest){
            const offset=rest.search(startPattern);
            if(offset<0){output.push({type:'text',value:rest});break;}
            if(offset>0){output.push({type:'text',value:rest.slice(0,offset)});rest=rest.slice(offset);}
            const token=matchInternalReference(rest);
            if(!token){output.push({type:'text',value:rest});break;}
            output.push({type:'link',url:token.href,children:[{type:'text',value:token.label}]});rest=rest.slice(token.raw.length);
          }
          return output;
        }
        visit(child);return [child];
      });
    };
    visit(tree);
  };
}
