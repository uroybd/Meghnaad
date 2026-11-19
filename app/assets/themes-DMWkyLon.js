var e=[{id:`default`,label:`Taskwarrior default`,note:`Taskwarrior's own default colours (the default)`},{id:`meghnaad`,label:`Meghnaad`,note:`soft colours for the web`},{id:`dark-16`,label:`Dark, 16 colours`},{id:`dark-256`,label:`Dark, 256 colours`},{id:`dark-blue-256`,label:`Dark blue`},{id:`dark-gray-256`,label:`Dark gray`},{id:`dark-gray-blue-256`,label:`Dark gray blue`},{id:`dark-green-256`,label:`Dark green`},{id:`dark-red-256`,label:`Dark red`},{id:`dark-violets-256`,label:`Dark violets`},{id:`dark-yellow-green`,label:`Dark yellow green`},{id:`bubblegum-256`,label:`Bubblegum`},{id:`light-16`,label:`Light, 16 colours`},{id:`light-256`,label:`Light, 256 colours`},{id:`solarized-dark-256`,label:`Solarized dark`},{id:`solarized-light-256`,label:`Solarized light`},{id:`no-color`,label:`No colour`,note:`every colour rule switched off`}],t=t=>e.some(e=>e.id===t),n=e=>/^\s*(color\.|rule\.precedence\.color|rule\.color\.merge)/.test(e);function r(e){return e.split(`
`).map(e=>e.split(`#`)[0].trim()).filter(Boolean)}var i=e=>`themes/${e}.rc`,a=e=>/^\s*include\s+themes\/[\w.-]+\.rc\s*(#.*)?$/.test(e);function o(e,t,r){let o=e.split(`
`).filter(e=>!n(e)&&!a(e)&&!/^# theme: /.test(e));for(;o.length&&o[o.length-1].trim()===``;)o.pop();return t==="default"?{main:o.join(`
`)+(o.length?`
`:``),theme:null}:{main:[...o,...o.length?[``]:[],`include ${i(t)}`].join(`
`)+`
`,theme:{path:i(t),text:r.endsWith(`
`)?r:`${r}\n`}}}function s(e){for(let n of e.split(`
`).reverse()){let e=/^\s*include\s+themes\/([\w.-]+)\.rc\s*(#.*)?$/.exec(n);if(e&&t(e[1]))return e[1]}return null}function c(e,t){let n=e.split(`
`).filter(e=>!/^\s*color\s*[=\s]/.test(e));for(;n.length&&n[n.length-1].trim()===``;)n.pop();return t||n.push(`color=off`),n.join(`
`)+(n.length?`
`:``)}function l(e){return e.split(`
`).some(e=>/^\s*color\s*[=\s]\s*(off|0|no|false|n)\s*$/i.test(e.split(`#`)[0]))}function u(e){let n=/^\s*include\s+(?:\S*\/)?([\w.-]+?)\.theme\s*$/.exec(e);return n&&t(n[1])?n[1]:null}async function d(e,t){let n=[];for(let i of e.split(`
`)){let e=u(i);e?n.push(`# theme: ${e}`,...r(await t(e))):n.push(i)}return n.join(`
`)}async function f(e){let t=await fetch(`themes/${e}.theme`);if(!t.ok)throw Error(`The ${e} theme could not be loaded.`);return t.text()}export{d as a,l as i,o as n,f as o,s as r,c as s,e as t};