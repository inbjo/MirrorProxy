import { readFileSync } from 'node:fs'
import assert from 'node:assert/strict'
const read = path => readFileSync(new URL(path, import.meta.url), 'utf8')
const rust = read('../../crates/server/src/config.rs')
const example = read('../../config.example.toml').split('[doh]')[1].split('\n[')[0]
const web = read('../src/config-defaults.ts')
const rustRate = Number(rust.match(/fn default_doh_rate_limit\(\) -> u32\s*\{\s*(\d+)/)[1])
assert.equal(Number(example.match(/requests_per_minute\s*=\s*(\d+)/)[1]), rustRate)
assert.equal(Number(web.match(/DOH_DEFAULT_REQUESTS_PER_MINUTE = (\d+)/)[1]), rustRate)
assert.match(example, /ecs_mode = "passthrough"/)
assert.match(rust, /fn default_doh_ecs\(\) -> DohEcsMode\s*\{\s*DohEcsMode::Passthrough/)
assert.match(web, /DOH_DEFAULT_ECS_MODE = 'passthrough'/)
console.log('DoH defaults agree across Rust, the console and the example configuration.')
