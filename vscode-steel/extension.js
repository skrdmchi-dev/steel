const vscode = require('vscode');
const cp = require('child_process');

/**
 * Steel VS Code Extension v1.0.0:
 * - Live Syntax & Type Error Squiggles (DiagnosticsCollection via `steel check`)
 * - Full IntelliSense Autocomplete (50+ Stdlib Built-ins, Method Chains & Keywords)
 * - Inferred Type Hover Cards
 * - Live Status Bar & One-Click --visual Multi-Target Compilation (C, C++, Rust, Python, Go, JS, TS, Lua)
 */
function activate(context) {
  const statusBar = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 100);
  statusBar.command = 'steel.compileVisual';
  context.subscriptions.push(statusBar);

  const diagCollection = vscode.languages.createDiagnosticCollection('steel');
  context.subscriptions.push(diagCollection);

  const stdlibDocs = {
    print: '`fn print(...args) -> null` - Prints formatted values to stdout.',
    println: '`fn println(...args) -> null` - Prints formatted values with newline.',
    input: '`fn input(prompt?: str) -> str` - Reads a line of input from stdin.',
    assert: '`fn assert(cond: bool, msg?: str) -> bool` - Throws an error if `cond` is false.',
    type: '`fn type(val: any) -> str` - Returns the runtime type name of a value.',
    str: '`fn str(val: any) -> str` - Converts any value to a string.',
    num: '`fn num(val: any) -> num` - Converts a string or boolean to a number.',
    len: '`fn len(x: array | str | map) -> num` - Returns length of collection or string.',
    push: '`fn push(arr: array, val: any) -> array` - Appends a value to an array.',
    pop: '`fn pop(arr: array) -> any` - Removes and returns the last element of an array.',
    keys: '`fn keys(m: map) -> array<str>` - Returns all keys in a map.',
    values: '`fn values(m: map) -> array<any>` - Returns all values in a map.',
    contains: '`fn contains(x: array | str | map, needle: any) -> bool` - Checks membership.',
    range: '`fn range(start: num, end: num, step?: num) -> array<num>` - Numeric range.',
    slice: '`fn slice(x: array | str, start: num, end?: num) -> array | str` - Extracts a slice.',
    reverse: '`fn reverse(x: array | str) -> array | str` - Reverses an array or string.',
    sort: '`fn sort(arr: array) -> array` - Returns a sorted copy of an array.',
    map: '`fn map(arr: array, fn: (x) => any) -> array` - Transforms each item in an array.',
    filter: '`fn filter(arr: array, fn: (x) => bool) -> array` - Filters items by predicate.',
    reduce: '`fn reduce(arr: array, fn: (acc, x) => any, init?: any) -> any` - Folds an array.',
    sum: '`fn sum(arr: array<num>) -> num` - Sums all numbers in an array.',
    avg: '`fn avg(arr: array<num>) -> num` - Computes the average of an array of numbers.',
    unique: '`fn unique(arr: array) -> array` - Deduplicates elements in an array.',
    flat: '`fn flat(arr: array) -> array` - Flattens one level of nested arrays.',
    enumerate: '`fn enumerate(arr: array) -> array<[index, item]>` - Pairs items with indices.',
    zip: '`fn zip(a: array, b: array) -> array<[a, b]>` - Zips two arrays together.',
    split: '`fn split(s: str, sep: str) -> array<str>` - Splits a string by separator.',
    join: '`fn join(arr: array, sep: str) -> str` - Joins array items with a separator.',
    trim: '`fn trim(s: str) -> str` - Trims leading and trailing whitespace.',
    upper: '`fn upper(s: str) -> str` - Converts string to uppercase.',
    lower: '`fn lower(s: str) -> str` - Converts string to lowercase.',
    replace: '`fn replace(s: str, from: str, to: str) -> str` - Replaces substrings.',
    json_parse: '`fn json_parse(s: str) -> any` - Parses a JSON string into Steel values.',
    json_stringify: '`fn json_stringify(val: any) -> str` - Serializes a Steel value to JSON.',
    args: '`fn args() -> array<str>` - Returns command-line arguments.',
    env: '`fn env(key: str) -> str | null` - Reads an environment variable.',
    set_env: '`fn set_env(key: str, val: str) -> bool` - Sets an environment variable.',
    cwd: '`fn cwd() -> str` - Returns the current working directory.',
    exec: '`fn exec(cmd: str) -> str` - Executes a shell command and returns stdout.',
    exit: '`fn exit(code?: num)` - Terminates the process with exit code.',
    file_exists: '`fn file_exists(path: str) -> bool` - Returns true if path exists.',
    read_file: '`fn read_file(path: str) -> str` - Reads file contents as UTF-8 string.',
    write_file: '`fn write_file(path: str, content: str) -> bool` - Writes string to file.',
    append_file: '`fn append_file(path: str, content: str) -> bool` - Appends string to file.',
    list_dir: '`fn list_dir(path: str) -> array<str>` - Lists filenames in a directory.',
    make_dir: '`fn make_dir(path: str) -> bool` - Creates a directory and parents.',
    remove_file: '`fn remove_file(path: str) -> bool` - Deletes a file.',
    clock: '`fn clock() -> num` - Current Unix timestamp in seconds.',
    sleep: '`fn sleep(ms: num) -> null` - Pauses execution for `ms` milliseconds.',
    random: '`fn random() -> num` - Random float in `[0.0, 1.0)`.',
    rand_int: '`fn rand_int(min: num, max: num) -> num` - Random integer in `[min, max)`.',
    hash: '`fn hash(s: str) -> str` - Computes fast 64-bit FNV-1a hex digest.',
    uuid: '`fn uuid() -> str` - Generates a random UUID v4 string.',
    abs: '`fn abs(x: num) -> num` - Absolute value.',
    sqrt: '`fn sqrt(x: num) -> num` - Square root.',
    pow: '`fn pow(a: num, b: num) -> num` - Exponentiation.',
    floor: '`fn floor(x: num) -> num` - Rounds down to integer.',
    ceil: '`fn ceil(x: num) -> num` - Rounds up to integer.',
    round: '`fn round(x: num) -> num` - Rounds to nearest integer.',
    min: '`fn min(a: num, b: num) -> num` - Minimum of two numbers.',
    max: '`fn max(a: num, b: num) -> num` - Maximum of two numbers.',
    clamp: '`fn clamp(x: num, lo: num, hi: num) -> num` - Clamps `x` between `lo` and `hi`.',
    sin: '`fn sin(x: num) -> num` - Sine (radians).',
    cos: '`fn cos(x: num) -> num` - Cosine (radians).',
    atan2: '`fn atan2(y: num, x: num) -> num` - Four-quadrant arctangent.',
    log: '`fn log(x: num) -> num` - Natural logarithm.',
    exp: '`fn exp(x: num) -> num` - Base-e exponential.',
    sign: '`fn sign(x: num) -> num` - Returns -1, 0, or 1.'
  };

  const os = require('os');
  const path = require('path');
  const fs = require('fs');

  function getSteelExe() {
    const candidate = path.join(os.homedir(), '.steel', 'bin', 'steel.exe');
    if (fs.existsSync(candidate)) return `"${candidate}"`;
    const cargoCandidate = path.join(os.homedir(), '.cargo', 'bin', 'steel.exe');
    if (fs.existsSync(cargoCandidate)) return `"${cargoCandidate}"`;
    return 'steel';
  }

  function runLiveDiagnostics(doc) {
    if (!doc || doc.languageId !== 'steel') return;
    const text = doc.getText();
    const lines = text.split('\n');
    const diags = [];

    // 1. Instant In-Memory Lint (works while typing even before Ctrl+S!)
    const defined = new Set([
      'print','println','input','assert','type','str','num','len','push','pop','keys','values',
      'contains','range','slice','reverse','sort','map','filter','reduce','sum','avg','unique',
      'flat','enumerate','zip','split','join','trim','upper','lower','replace','json_parse',
      'json_stringify','args','env','set_env','cwd','exec','exit','file_exists','read_file',
      'write_file','append_file','list_dir','make_dir','remove_file','clock','sleep','random',
      'rand_int','hash','uuid','abs','sqrt','pow','floor','ceil','round','min','max','clamp',
      'sin','cos','atan2','log','exp','sign','PI','E','this','self','true','false','null','nil'
    ]);
    const fnNames = new Set();

    // First pass: collect declared functions, classes, enums, params, and let/const variables
    for (let i = 0; i < lines.length; i++) {
      const cleanLine = lines[i].replace(/\/\/.*$/, '');
      let m = cleanLine.match(/\bfn\s+([A-Za-z_]\w*)\s*\(([^)]*)\)/);
      if (m) {
        defined.add(m[1]);
        fnNames.add(m[1]);
        m[2].split(',').forEach(p => {
          const pName = p.trim().split(/[:=]/)[0].replace(/^\.\.\./, '').trim();
          if (pName) defined.add(pName);
        });
      }
      const clsMatch = cleanLine.match(/\b(?:class|enum)\s+([A-Za-z_]\w*)/);
      if (clsMatch) defined.add(clsMatch[1]);
      const letMatch = cleanLine.match(/\b(?:let|const|for)\s+([A-Za-z_]\w*)/);
      if (letMatch) defined.add(letMatch[1]);
      const catchMatch = cleanLine.match(/\bcatch\s*\(?\s*([A-Za-z_]\w*)/);
      if (catchMatch) defined.add(catchMatch[1]);
    }

    // Second pass: check for overwriting a function name without `let` and undefined identifiers in calls/assignments
    for (let i = 0; i < lines.length; i++) {
      const rawLine = lines[i];
      const cleanLine = rawLine.replace(/\/\/.*$/, '').replace(/"([^"\\]|\\.)*"/g, '""').replace(/'([^'\\]|\\.)*'/g, "''");

      // Check `fn_name = ...` without `let`
      const assignMatch = cleanLine.match(/^\s*([A-Za-z_]\w*)\s*=(?!=)/);
      if (assignMatch) {
        const lhs = assignMatch[1];
        const col = rawLine.indexOf(lhs);
        if (fnNames.has(lhs)) {
          diags.push(new vscode.Diagnostic(
            new vscode.Range(i, col, i, col + lhs.length),
            `Cannot overwrite function '${lhs}' without 'let' (use 'let result = ${lhs}(...)')`,
            vscode.DiagnosticSeverity.Error
          ));
        } else if (!defined.has(lhs)) {
          diags.push(new vscode.Diagnostic(
            new vscode.Range(i, col, i, col + lhs.length),
            `Variable '${lhs}' assigned without 'let' declaration (use 'let ${lhs} = ...')`,
            vscode.DiagnosticSeverity.Warning
          ));
          defined.add(lhs);
        }
      }

      // Check identifiers passed as arguments e.g. `emotion(sad)` where `sad` is not defined
      const callArgRegex = /\b[A-Za-z_]\w*\s*\(([^()]*)\)/g;
      let cm;
      while ((cm = callArgRegex.exec(cleanLine)) !== null) {
        const isFnDecl = /\bfn\s+/.test(cleanLine);
        if (isFnDecl) continue;
        const argList = cm[1].split(',');
        for (const rawArg of argList) {
          const arg = rawArg.trim();
          if (/^[A-Za-z_]\w*$/.test(arg) && !defined.has(arg)) {
            const col = rawLine.lastIndexOf(arg);
            diags.push(new vscode.Diagnostic(
              new vscode.Range(i, Math.max(0, col), i, Math.max(0, col) + arg.length),
              `Undefined variable '${arg}' (did you mean string "${arg}"?)`,
              vscode.DiagnosticSeverity.Error
            ));
          }
        }
      }
    }

    // 2. Also run `steel check` CLI on saved disk file for full parser diagnostics
    if (!doc.isDirty) {
      cp.exec(`${getSteelExe()} check "${doc.fileName}"`, (err, stdout, stderr) => {
        if (err) {
          const combined = ((stderr || '') + '\n' + (stdout || '')).replace(/\x1b\[[0-9;]*m/g, '');
          const coordMatch = combined.match(/at\s+(\d+):(\d+)/);
          const errLines = combined.split('\n').filter(l => l.trim().startsWith('error:'));
          for (const el of errLines) {
            const msg = el.replace(/^error:\s*/, '').trim();
            let line = coordMatch ? Math.max(0, parseInt(coordMatch[1], 10) - 1) : 0;
            let col = coordMatch ? Math.max(0, parseInt(coordMatch[2], 10) - 1) : 0;
            const varM = msg.match(/'([^']+)'/);
            if (varM) {
              const needle = varM[1];
              for (let li = 0; li < lines.length; li++) {
                const idx = lines[li].indexOf(needle);
                if (idx !== -1) { line = li; col = idx; break; }
              }
            }
            if (!diags.some(d => d.range.start.line === line && d.message.includes(msg))) {
              diags.push(new vscode.Diagnostic(
                new vscode.Range(line, col, line, col + 6),
                msg,
                vscode.DiagnosticSeverity.Error
              ));
            }
          }
        }
        diagCollection.set(doc.uri, diags);
      });
    } else {
      diagCollection.set(doc.uri, diags);
    }
  }

  function updateSteelAnalysis(doc) {
    if (!doc || doc.languageId !== 'steel') {
      statusBar.hide();
      return;
    }
    const text = doc.getText();
    const fnCount = (text.match(/\bfn\s+[A-Za-z_]\w*/g) || []).length;
    const classCount = (text.match(/\b(class|enum)\s+[A-Za-z_]\w*/g) || []).length;
    const pipeCount = (text.match(/\|>/g) || []).length;
    const testCount = (text.match(/\btest\s+"/g) || []).length;

    statusBar.text = `$(flame) Steel: ${fnCount} fn | ${classCount} types | ${pipeCount} |> | ${testCount} tests`;
    statusBar.tooltip = 'Click to Compile with --visual Morphing (C, C++, Rust, Python, Go, JS, TS, Lua)';
    statusBar.show();
  }

  // 1. IntelliSense Autocomplete Provider
  const completionProvider = vscode.languages.registerCompletionItemProvider(
    'steel',
    {
      provideCompletionItems() {
        const items = [];
        const keywords = [
          'fn', 'let', 'const', 'class', 'extends', 'enum', 'new', 'return',
          'if', 'else', 'while', 'for', 'in', 'break', 'continue', 'match',
          'try', 'catch', 'throw', 'defer', 'test', 'import', 'true', 'false', 'null'
        ];
        for (const kw of keywords) {
          items.push(new vscode.CompletionItem(kw, vscode.CompletionItemKind.Keyword));
        }
        for (const [fnName, docStr] of Object.entries(stdlibDocs)) {
          const item = new vscode.CompletionItem(fnName, vscode.CompletionItemKind.Function);
          item.documentation = new vscode.MarkdownString(docStr);
          items.push(item);
        }
        return items;
      }
    },
    '.'
  );

  // 2. Hover Type Intelligence Provider
  const hoverProvider = vscode.languages.registerHoverProvider('steel', {
    provideHover(document, position) {
      const range = document.getWordRangeAtPosition(position);
      if (!range) return null;
      const word = document.getText(range);

      if (stdlibDocs[word]) {
        return new vscode.Hover(new vscode.MarkdownString(`**Steel Standard Library**\n\n${stdlibDocs[word]}`));
      }

      const text = document.getText();
      const fnRegex = new RegExp(`\\bfn\\s+(${word})\\s*\\(([^)]*)\\)(?:\\s*->\\s*(\\w+))?`);
      const fnMatch = text.match(fnRegex);
      if (fnMatch) {
        const ret = fnMatch[3] || 'auto (inferred)';
        return new vscode.Hover(
          new vscode.MarkdownString(`**Steel Function**\n\`\`\`steel\nfn ${fnMatch[1]}(${fnMatch[2]}) -> ${ret}\n\`\`\``)
        );
      }

      const letRegex = new RegExp(`\\b(let|const)\\s+(${word})(?:\\s*:\\s*(\\w+))?\\s*=\\s*([^\\n;]+)`);
      const letMatch = text.match(letRegex);
      if (letMatch) {
        const rhs = letMatch[4].trim();
        let inferred = letMatch[3] || 'dynamic (SVal)';
        if (!letMatch[3]) {
          if (/^[\d_]+(\.[\d_]+)?$/.test(rhs)) inferred = 'num (unboxed f64)';
          else if (/^["']/.test(rhs)) inferred = 'str';
          else if (/^(true|false)$/.test(rhs)) inferred = 'bool';
          else if (/^\[/.test(rhs)) inferred = 'array';
          else if (/^\{/.test(rhs)) inferred = 'map';
          else if (/^new\s+(\w+)/.test(rhs)) inferred = `class<${rhs.match(/^new\s+(\w+)/)[1]}>`;
        }
        return new vscode.Hover(
          new vscode.MarkdownString(`**Steel Symbol** (\`${letMatch[1]} ${word}\`)\n- **Identified Type**: \`${inferred}\`\n- **Initializer**: \`${rhs}\``)
        );
      }
      return null;
    }
  });

  // 3. Go-to-Definition Provider (F12 / Ctrl+Click on functions, classes, enums, and variables)
  const definitionProvider = vscode.languages.registerDefinitionProvider('steel', {
    provideDefinition(document, position) {
      const range = document.getWordRangeAtPosition(position);
      if (!range) return null;
      const word = document.getText(range);
      const lines = document.getText().split('\n');
      const declRegex = new RegExp(`^\\s*(?:fn|class|enum|let|const)\\s+(${word})\\b`);

      for (let i = 0; i < lines.length; i++) {
        const m = lines[i].match(declRegex);
        if (m) {
          const col = lines[i].indexOf(word);
          return new vscode.Location(document.uri, new vscode.Position(i, Math.max(0, col)));
        }
      }
      return null;
    }
  });

  // 4. Document Outline & Breadcrumbs Provider (DocumentSymbolProvider)
  const symbolProvider = vscode.languages.registerDocumentSymbolProvider('steel', {
    provideDocumentSymbols(document) {
      const symbols = [];
      const lines = document.getText().split('\n');

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i];
        let m = line.match(/^\s*fn\s+([A-Za-z_]\w*)\s*\(([^)]*)\)/);
        if (m) {
          const pos = new vscode.Range(i, 0, i, line.length);
          symbols.push(new vscode.DocumentSymbol(m[1], `(${m[2]})`, vscode.SymbolKind.Function, pos, pos));
          continue;
        }
        m = line.match(/^\s*class\s+([A-Za-z_]\w*)/);
        if (m) {
          const pos = new vscode.Range(i, 0, i, line.length);
          symbols.push(new vscode.DocumentSymbol(m[1], 'class', vscode.SymbolKind.Class, pos, pos));
          continue;
        }
        m = line.match(/^\s*enum\s+([A-Za-z_]\w*)/);
        if (m) {
          const pos = new vscode.Range(i, 0, i, line.length);
          symbols.push(new vscode.DocumentSymbol(m[1], 'enum', vscode.SymbolKind.Enum, pos, pos));
          continue;
        }
        m = line.match(/^\s*test\s+"([^"]+)"/);
        if (m) {
          const pos = new vscode.Range(i, 0, i, line.length);
          symbols.push(new vscode.DocumentSymbol(`test: ${m[1]}`, 'unit test', vscode.SymbolKind.Event, pos, pos));
        }
      }
      return symbols;
    }
  });

  const runCmd = vscode.commands.registerCommand('steel.runFile', () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;
    const filePath = editor.document.fileName;
    const term = vscode.window.activeTerminal || vscode.window.createTerminal('Steel');
    term.show();
    term.sendText(`steel run "${filePath}"`);
  });

  const compileVisualCmd = vscode.commands.registerCommand('steel.compileVisual', async () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;
    const filePath = editor.document.fileName;
    const target = await vscode.window.showQuickPick(
      [
        { label: 'c', description: 'C11 (Unboxed Math + SVal Runtime)' },
        { label: 'cpp', description: 'C++ (C++20)' },
        { label: 'rust', description: 'Rust (Standalone SVal Binary)' },
        { label: 'python', description: 'Python 3' },
        { label: 'go', description: 'Go (Golang)' },
        { label: 'js', description: 'JavaScript (Node.js)' },
        { label: 'ts', description: 'TypeScript' },
        { label: 'lua', description: 'Lua' },
        { label: 'all', description: 'Compile to all 8 target languages at once!' }
      ],
      { placeHolder: 'Choose target language for `steel compile <lang> --visual --force`' }
    );
    if (!target) return;
    const term = vscode.window.activeTerminal || vscode.window.createTerminal('Steel Visual Compiler');
    term.show();
    term.sendText(`steel compile ${target.label} "${filePath}" --visual --force`);
  });

  const testCmd = vscode.commands.registerCommand('steel.runTests', () => {
    const editor = vscode.window.activeTextEditor;
    if (!editor) return;
    const filePath = editor.document.fileName;
    const term = vscode.window.activeTerminal || vscode.window.createTerminal('Steel Tests');
    term.show();
    term.sendText(`steel test "${filePath}"`);
  });

  context.subscriptions.push(
    completionProvider,
    hoverProvider,
    definitionProvider,
    symbolProvider,
    runCmd,
    compileVisualCmd,
    testCmd,
    vscode.window.onDidChangeActiveTextEditor(ed => {
      updateSteelAnalysis(ed && ed.document);
      if (ed && ed.document) runLiveDiagnostics(ed.document);
    }),
    vscode.workspace.onDidChangeTextDocument(e => {
      updateSteelAnalysis(e.document);
      runLiveDiagnostics(e.document);
    }),
    vscode.workspace.onDidSaveTextDocument(doc => runLiveDiagnostics(doc))
  );

  if (vscode.window.activeTextEditor) {
    updateSteelAnalysis(vscode.window.activeTextEditor.document);
    runLiveDiagnostics(vscode.window.activeTextEditor.document);
  }
}

function deactivate() {}

module.exports = { activate, deactivate };
