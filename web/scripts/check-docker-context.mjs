import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

const stylesheet = readFileSync(new URL('../src/styles.css', import.meta.url), 'utf8')
const dockerfile = readFileSync(new URL('../../Dockerfile', import.meta.url), 'utf8')

assert.match(stylesheet, /@import\s+["']\.\.\/\.\.\/tokens\.css["']/)
assert.match(dockerfile, /COPY\s+tokens\.css\s+\/app\/tokens\.css/)

console.log('Docker web-build context includes the root design tokens.')
