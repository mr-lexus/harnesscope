# Руководство разработчика (Harnesscope)

В этом документе описаны требования, структура проекта, запуск в dev-режиме, тестирование и добавление новых runner-адаптеров.

---

## 1. Системные требования

- **Rust toolchain**: 1.80+ (`cargo`, `rustc`)
- **Node.js**: v20.19+ или v22.12+ с `npm` (рекомендуется Node.js 24 LTS)
- **Git**: 2.30+ установлен и доступен в PATH
- **Поддерживаемые ОС**: Windows 10/11, macOS (Intel/Apple Silicon), Linux (Ubuntu, Debian, Fedora, Arch)

---

## 2. Структура проекта

```
harnesscope/
├── Cargo.toml               # Конфигурация Rust и зависимости
├── src/
│   ├── main.rs              # Точка входа CLI и запуск сервера
│   ├── lib.rs               # Экспорт библиотеки для тестов
│   ├── cli.rs               # Определение команд clap
│   ├── config.rs            # Пути данных и конфигурация портов
│   ├── redact.rs            # Редактирование секретов и SHA-256
│   ├── git.rs               # Детерминированный сбор Git-контекста
│   ├── demo.rs              # Генератор детерминированных демо-данных
│   ├── domain/              # Доменные сущности и события
│   │   ├── models.rs        # RuntimeInstance, Session, Execution и др.
│   │   ├── events.rs        # IngestEvent и IngestPayload
│   │   └── mod.rs
│   ├── storage/             # Слой хранения (SQLite)
│   │   ├── db.rs            # Настройка подключения (WAL, busy_timeout)
│   │   ├── migrations.rs    # Версионирование миграций схемы
│   │   ├── repository.rs    # SQL-запросы, фильтрация и метрики
│   │   └── mod.rs
│   ├── server/              # Сервер Axum HTTP API и Web UI
│   │   ├── correlation.rs   # Движок корреляции: сессии и атрибуция
│   │   ├── handlers.rs      # Обработчики REST API
│   │   ├── routes.rs        # Таблица маршрутов и CORS
│   │   ├── static_files.rs  # Отдача встроенных файлов SPA
│   │   └── mod.rs
│   └── runners/             # Прозрачные обёртки агентов
│       ├── discovery.rs     # Поиск исполняемых файлов (PATH и env)
│       ├── wrapper.rs       # Проброс stdio, TTY, Ctrl+C и кодов возврата
│       ├── codex.rs         # Парсер аргументов и конфигов Codex
│       ├── copilot.rs       # Парсер аргументов и конфигов Copilot
│       ├── opencode.rs      # Парсер OpenCode CLI/TUI и OMO
│       └── mod.rs
├── tests/
│   └── harnesscope_tests.rs # Полный набор интеграционных тестов
├── web/                     # Фронтенд (React + Vite + Mantine + FSD)
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── index.html
│   └── src/
│       ├── app/             # Корневой каркас и провайдеры
│       ├── pages/           # Executions, ExecutionDetail, Sessions, SessionDetail, Stats
│       ├── widgets/         # Header и навигация
│       ├── features/        # Фильтры и кнопка сидирования демо
│       ├── entities/        # Интерфейсы TypeScript
│       └── shared/          # Клиент API, бейджи и утилиты
└── docs/                    # Документация
    ├── ARCHITECTURE.md
    ├── DEVELOPMENT.md
    └── DEVELOPMENT.ru.md
```

---

## 3. Разработка и сборка

### Разработка фронтенда
Для запуска Vite с горячей перезагрузкой:
```bash
cd web
npm install
npm run dev
```
Vite доступен на `http://localhost:5173` и автоматически проксирует `/api` на `http://127.0.0.1:4242`.

Сборка production-бандла фронтенда:
```bash
cd web
npm run build
```
Скомпилированные файлы помещаются в `web/dist/` и встраиваются в итоговый бинарник Rust через `rust-embed`.

### Разработка бэкенда
Запуск сервера в режиме разработки:
```bash
cargo run -- serve --port 4242
```
Быстрая проверка типов и компиляции:
```bash
cargo check
```

Сборка production release:
```bash
cd web && npm run build && cd ..
cargo build --release
```
Готовый бинарник находится в `target/release/harnesscope` (`harnesscope.exe` на Windows).

---

## 4. Запуск тестов

Запуск всех интеграционных тестов:
```bash
cargo test
```

Тестовый набор покрывает все обязательные сценарии:
- Миграции схемы SQLite;
- Конкурентная запись событий (безопасность блокировок WAL);
- Два одновременных execution в разных worktree;
- Параллельные execution в одном worktree (атрибуция `AMBIGUOUS`);
- Resume сессии после остановки процесса (`process stopped != session stopped`);
- Раздельность сессий в одном worktree/branch при отсутствии доказательства связи;
- Вырезание секретов (OpenAI, Anthropic, GitHub токены, пароли, Bearer-токены);
- Устойчивость к неизвестным полям событий;
- Падение runner и сохранение кода ошибки;
- Прозрачный проброс кода возврата.

---

## 5. Расположение и сброс базы данных

- **Windows**: `%LOCALAPPDATA%\harnesscope\harnesscope\data\harnesscope.db`
- **Linux**: `~/.local/share/harnesscope/harnesscope.db`
- **macOS**: `~/Library/Application Support/com.harnesscope.harnesscope/harnesscope.db`
- **Пользовательский путь**: переменная `HARNESSCOPE_DB_PATH=/path/to/custom.db`.

Для полного сброса базы достаточно удалить файл `.db`:
```bash
# Windows PowerShell
Remove-Item "$env:LOCALAPPDATA\harnesscope\harnesscope\data\harnesscope.db*"
# Linux / macOS
rm -f ~/.local/share/harnesscope/harnesscope.db*
```

---

## 6. Как работают миграции

Миграции управляются в `src/storage/migrations.rs`.
- Служебная таблица `_schema_migrations` фиксирует применённые версии.
- При вызове `Database::open` запускается `run_migrations`, проверяет текущую версию и применяет новые шаги последовательно в коротких транзакциях.

---

## 7. Как добавить новый runner-адаптер

1. **Создайте парсер** в `src/runners/<runner>.rs`:
   - Реализуйте `parse_<runner>_args_and_env(args: &[String], cwd: &Path) -> RunnerMetadata`.
   - Извлеките нативные session ID, флаги модели, reasoning effort, роли агента.
   - Прочитайте локальные конфигурационные файлы (если есть), зафиксируйте MCP/скиллы/плагины в статусе `CONFIGURED`.
   - Если факт неизвестен — ставьте `"UNKNOWN"`.
2. **Добавьте обнаружение** в `src/runners/discovery.rs`:
   - Добавьте проверку переменной окружения `HARNESSCOPE_<RUNNER>_BIN`.
   - Добавьте стандартные имена бинарников для поиска в PATH.
3. **Зарегистрируйте подкоманду CLI** в `src/cli.rs`.
4. **Свяжите вызов в `src/main.rs`** через `execute_wrapper`.
5. **Добавьте тесты** парсера аргументов и конфигураций.
