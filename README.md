# V8Unpack for RUST

The project is based on [dmpas/v8unpack](https://github.com/dmpas/v8unpack)
and is a partially ported version of the parser.

The task was to try to write the parser files `1cd` to RUST.

The project is divided into 2 parts to allow use from other languages
through `FFI`.

- `v8unpack4rs` - the library.
- `v8unpack` - command line utility.

## Russian language

Проект основан на базе [dmpas/v8unpack](https://github.com/dmpas/v8unpack)
и является частично портированной версией парсера.

Задача была попробовать написать парсер файлов `1cd` на RUST.

Проект разделен на 2 части для обеспечения возможности использования из других языков
через интерфейс внешних функций.

- `v8unpack4rs` - библиотека.
- `v8unpack` - консольная программа.