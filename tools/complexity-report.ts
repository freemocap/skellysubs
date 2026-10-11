/** Advisory syntax metrics, not algorithmic complexity or a runtime cost estimate. */
import ts from 'typescript'
import { readdir, readFile } from 'node:fs/promises'
import { relative, resolve, join } from 'node:path'

export type FunctionMetric = { file:string; line:number; name:string; lines:number; decisions:number; maxNesting:number }
export function measureFunctions(file:string, text:string): FunctionMetric[] {
  const source=ts.createSourceFile(file,text,ts.ScriptTarget.Latest,true,file.endsWith('.tsx')?ts.ScriptKind.TSX:ts.ScriptKind.TS)
  const functions:FunctionMetric[]=[]
  function walk(node:ts.Node) {
    if (ts.isFunctionLike(node) && 'body' in node && node.body) {
      let decisions=0,maxNesting=0
      function body(child:ts.Node,depth:number) {
        if (ts.isFunctionLike(child)) return // Nested functions get independent metrics.
        const branch=ts.isIfStatement(child)||ts.isConditionalExpression(child)||ts.isForStatement(child)||
          ts.isForInStatement(child)||ts.isForOfStatement(child)||ts.isWhileStatement(child)||ts.isDoStatement(child)||
          ts.isCatchClause(child)||ts.isCaseClause(child)
        const logical=ts.isBinaryExpression(child)&&[ts.SyntaxKind.AmpersandAmpersandToken,ts.SyntaxKind.BarBarToken,ts.SyntaxKind.QuestionQuestionToken].includes(child.operatorToken.kind)
        if(branch||logical)decisions++
        const next=depth+(branch?1:0);maxNesting=Math.max(maxNesting,next)
        ts.forEachChild(child,c=>body(c,next))
      }
      body(node.body,0)
      const start=source.getLineAndCharacterOfPosition(node.getStart(source)).line
      const end=source.getLineAndCharacterOfPosition(node.end).line
      const name=node.name?.getText(source) ?? (ts.isVariableDeclaration(node.parent)?node.parent.name.getText(source):'<anonymous>')
      functions.push({file,line:start+1,name,lines:end-start+1,decisions,maxNesting})
    }
    ts.forEachChild(node,walk)
  }
  walk(source);return functions
}

export async function complexityReport(roots:string[]) {
  const functions:FunctionMetric[]=[]
  const files:{file:string;lines:number;functionMetrics:boolean}[]=[]
  async function visit(directory:string) {
    for(const entry of await readdir(directory,{withFileTypes:true})) {
      if(entry.isSymbolicLink())throw Error('Source metric scan refuses links.')
      const path=join(directory,entry.name)
      if(entry.isDirectory()) {
        if(!['node_modules','target','dist','generated','.git','.local','tests','__tests__'].includes(entry.name))await visit(path)
      }else if(entry.isFile()&&/\.(tsx?|rs)$/.test(entry.name)&&!/(\.test\.|\.spec\.|\.d\.ts$)/.test(entry.name)){
        const text=await readFile(path,'utf8');const file=relative(process.cwd(),path).replaceAll('\\','/')
        files.push({file,lines:text.split(/\r?\n/).length,functionMetrics:!file.endsWith('.rs')})
        if(!file.endsWith('.rs'))functions.push(...measureFunctions(file,text))
      }
    }
  }
  for(const root of roots)await visit(resolve(root))
  return {format:1,policy:'Advisory only; no gate. Decision count is a documented syntax count, not formal cyclomatic complexity or Big-O.',
    counting:'Each if, conditional expression, loop, catch, non-default case and &&/||/?? counts once. Nested functions counted independently. Nesting counts branch/loop/catch/case containment.',
    coverage:'TypeScript function metrics; Rust line counts only. Generated files and separately located tests excluded; colocated Rust test modules remain in file line counts.',
    fileCount:files.length,functionCount:functions.length,
    files:files.sort((a,b)=>b.lines-a.lines||a.file.localeCompare(b.file)),
    functions:functions.sort((a,b)=>b.decisions-a.decisions||b.maxNesting-a.maxNesting||a.file.localeCompare(b.file)||a.line-b.line)}
}
if(import.meta.main)console.log(JSON.stringify(await complexityReport(process.argv.length>2?process.argv.slice(2):['ui/src','native/src','tools']),null,2))
