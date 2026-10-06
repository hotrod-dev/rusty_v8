module.exports = function runCorpus() {
  const output = [];
  function read(object, key) { return object[key]; }
  const shapes = Array.from({ length: 32 }, (_, i) => ({ ['shape' + i]: i }));
  // Exercise one keyed site across shapes and names before the semantic probes.
  const keys = Array.from({ length: 64 }, (_, i) => 'missing_' + i);
  const intern = Object.fromEntries(keys.map(key => [key, 0]));
  for (let i = 0; i < 16000; i++) {
    if (read(shapes[i & 31], keys[i & 63]) !== undefined) throw Error('warmup');
  }
  function observe(name, callback) {
    try {
      const value = callback();
      output.push([name, value === undefined ? '<undefined>' : value]);
    } catch (error) { output.push([name, '<throw>', error.name, error.message]); }
  }
  const plain = { stable: 1 };
  observe('missing', () => read(plain, 'missing'));
  plain.missing = 42;
  observe('added', () => read(plain, 'missing'));
  delete plain.missing;
  observe('deleted', () => read(plain, 'missing'));
  const parent = { inherited: 9 };
  const child = Object.create(parent);
  child.own = 1;
  observe('inherited data', () => read(child, 'inherited'));
  observe('inherited missing', () => read(child, 'later'));
  parent.later = 7;
  observe('prototype add', () => read(child, 'later'));
  delete parent.later;
  observe('prototype delete', () => read(child, 'later'));
  Object.setPrototypeOf(child, { later: 8 });
  observe('prototype replacement', () => read(child, 'later'));
  Object.setPrototypeOf(child, null);
  observe('null prototype', () => read(child, 'later'));
  let calls = 0;
  const accessor = Object.create({ get selected() { calls++; return this.answer; } });
  accessor.answer = 31;
  observe('getter receiver', () => read(accessor, 'selected'));
  observe('getter calls', () => calls);
  Object.defineProperty(accessor, 'selected', { get() { calls++; return undefined; }, configurable: true });
  observe('own undefined getter', () => read(accessor, 'selected'));
  observe('getter calls after own', () => calls);
  delete accessor.selected;
  observe('revealed getter', () => read(accessor, 'selected'));
  observe('getter calls after delete', () => calls);
  const throwing = Object.create({ get selected() { calls++; throw new Error('getter failure'); } });
  observe('throwing getter', () => read(throwing, 'selected'));
  observe('throwing getter calls', () => calls);
  const setterOnly = Object.create({ set selected(value) { throw new Error('setter called'); } });
  observe('setter only', () => read(setterOnly, 'selected'));
  const events = [];
  const proxy = new Proxy({}, {
    get(target, key, receiver) { events.push(['get', String(key), receiver === proxyChild]); return 19; },
    getOwnPropertyDescriptor() { throw new Error('descriptor trap called'); },
    getPrototypeOf() { throw new Error('prototype trap called'); }
  });
  const proxyChild = Object.create(proxy);
  proxyChild.own = 2;
  observe('proxy in chain', () => read(proxyChild, 'selected'));
  observe('proxy events', () => events.slice());
  const revoked = Proxy.revocable({}, {});
  const revokedChild = Object.create(revoked.proxy);
  revoked.revoke();
  // Error wording differs across V8/Node versions; compare the error type.
  observe('revoked proxy', () => { try { read(revokedChild, 'selected'); } catch (e) { return e.name; } });
  const symbol = Symbol('property');
  const symbolParent = { [symbol]: 23 };
  const symbolChild = Object.create(symbolParent);
  observe('inherited symbol', () => read(symbolChild, symbol));
  delete symbolParent[symbol];
  observe('deleted symbol', () => read(symbolChild, symbol));
  let conversions = 0;
  const key = { [Symbol.toPrimitive]() { conversions++; return 'selected'; } };
  observe('key coercion', () => read(accessor, key));
  observe('key coercion count', () => conversions);
  const typed = new Uint8Array([11, 12]);
  const typedChild = Object.create(typed);
  for (const name of ['0', '1', '2', '-0', 'NaN', 'Infinity', '1.5', 'selected']) {
    observe('typed own ' + name, () => read(typed, name));
    observe('typed prototype ' + name, () => read(typedChild, name));
  }
  const dictionary = Object.fromEntries(Array.from({ length: 180 }, (_, i) => ['entry' + i, i]));
  for (let i = 0; i < 90; i++) delete dictionary['entry' + i];
  const dictionaryChild = Object.create(dictionary);
  observe('dictionary inherited', () => read(dictionaryChild, 'entry170'));
  observe('dictionary absent', () => read(dictionaryChild, 'entry10'));
  dictionary.entry10 = 13;
  observe('dictionary added', () => read(dictionaryChild, 'entry10'));
  class Private { #value = 51; read() { return this.#value; } }
  observe('private field', () => new Private().read());
  observe('private brand', () => { try { Private.prototype.read.call({}); } catch (e) { return e.name; } });
  Object.freeze(plain);
  observe('frozen absent', () => read(plain, 'later'));
  for (const name of ['', '雪', 'x'.repeat(1500)]) {
    observe('unusual missing ' + name.length, () => read(plain, name));
  }
  observe('intern table intact', () => Object.keys(intern).length);
  return output;
};
