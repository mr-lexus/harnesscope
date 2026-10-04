# Данные ретроспективы, API и MCP

[Документация](../README.ru.md#документация)

Перед оценкой полноты прочитайте [матрицу возможностей](CAPABILITIES.ru.md). Версия API 1 и формат данных 1 отличаются от версии схемы SQLite 7. Существующий `/api/v1/events` работает без изменений.

## Подключение

```sh
harnesscope sources add-codex --path /absolute/home/.codex/sessions --include-content
harnesscope sources add-codex --path /absolute/home/.codex/archived_sessions --include-content
# Необязательно: читается только известная таблица истории в режиме read-only.
harnesscope sources add-codex --path /absolute/home/.codex/thread_history_1.sqlite --include-content
harnesscope capture workflow /absolute/project
harnesscope capture setup --codex-home /absolute/home/.codex
harnesscope serve
```

В Sources можно включить очищенное содержимое у существующей регистрации и повторно прочитать оригиналы. Для смены политики не нужно отключать единственный источник. При обновлении v5/v6 перед миграцией создаётся `*.before-v7-*.db`.

Чтобы связывать наблюдаемое чтение задач с задачами и версиями, зарегистрируйте локальный [универсальный адаптер задач](TASK_ADAPTERS.ru.md). Встроенных провайдеров нет.

`capture setup` добавляет свои точные обработчики в существующие массивы hooks, а локальный OTel JSON — только при отсутствии `[otel]`. Существующий OTel сохраняется; доверие к hooks не включается и не обходится. **Проверьте hooks и подтвердите доверие в штатном UI Codex.** Повторный setup с `--remove`, тем же исполняемым файлом и портом удаляет неизменённые собственные обработчики и точно добавленный блок OTel. Изменённые пользователем блоки сохраняются. `capture config` печатает предлагаемый JSON hooks без установки. Для другого порта задайте `HARNESSCOPE_SERVER_PORT` перед setup.

Hook и сервер должны использовать одну БД. Для нестандартного пути задайте `HARNESSCOPE_DB_PATH` перед setup: созданные hooks фиксируют разрешённый абсолютный путь через `capture hook --database` независимо от окружения запуска Codex. Очередь: `<database>.capture`. `capture drain` повторяет офлайн-сбор. Hooks не выдают в stdout решения allow/deny или инструкции.

На macOS `bash scripts/install-macos.sh /path/to/macOS/harnesscope` устанавливает пользовательский LaunchAgent и регистрирует активную/архивную историю в режиме метаданных. Включите очищенное содержимое в Sources, затем настройте hooks и доверие. Этот скрипт ещё не прошёл приёмку на настоящем macOS GUI. Установка Windows и запуск при входе пользователя продолжают поддерживаться.

## HTTP, только loopback

| Метод / путь | Контракт |
| --- | --- |
| GET `/api/v1/evidence` | Страница метаданных; `after`, `through`, `limit` (1–200), `session`, `task`, `project`, `kind`, `since`, `until`, `q` |
| GET `/api/v1/evidence/objects/:hash` | Полный очищенный объект с проверкой SHA-256, загружается явно по запросу |
| GET `/api/v1/evidence/objects/:hash/pages` | Ограниченная порция JSON-текста; `offset` в байтах UTF-8, `limit` (1–32000), ответ `next` |
| GET `/api/v1/evidence/diff?before=HASH&after=HASH` | Ограниченное сравнение по общему префиксу/суффиксу; диапазоны изменений, не минимальный набор правок |
| GET `/api/v1/evidence/context` | Страницы контекста, compaction и usage с явной достоверностью |
| GET `/api/v1/evidence/coverage` | Счётчики каналов, исключения и заявленные ограничения |
| GET `/api/v1/evidence/export` | Загрузка зафиксированного JSONL-пакета с теми же фильтрами |
| GET/POST `/api/v1/tasks` | Список (`after` — ID задачи, `limit`) / создание `{title,project?}` |
| GET `/api/v1/tasks/:id` | Связи, человеческие отметки, наблюдаемые внешние версии |
| POST `/api/v1/tasks/:id/links` | `{session_id,turn_id?,confirmed}` с **нативными** ID |
| POST `/api/v1/tasks/:id/marks` | `{kind,note}`; accepted/rework/repeated_mistake/misunderstood/successful_approach |
| GET/POST `/api/v1/workflow` | Страница версий (`root`, `after` — курсор строки) / регистрация `{path}` |
| POST `/api/v1/capture/hooks` | JSON структуры hook; подтверждение надёжной записи в БД |
| POST `/v1/logs`, `/v1/traces` | OTLP/HTTP **JSON**, до 500 записей / 2 MiB; без protobuf/gRPC |

Сохраняйте `through` при переходе между страницами хронологии. `q` ищет по метаданным событий/инструментов/сессий, не по всем payload. Тексты объектов намеренно не включены в списки. Страница workflow содержит до 100 записей; продолжайте с последним `cursor`. Карточка задачи показывает до 200 внешних версий; полную историю читайте через страницы наблюдений с фильтром задачи. Селекторы UI показывают первые 200 задач; API/MCP поддерживает пагинацию по ID.

Статусы архива: `retained`, `redacted`, `excluded`. У каждого исключения есть причина. Ошибки нативных проекций видны в Sources: покрытие нужно оценивать вместе с диагностикой, не только по количеству записей.

## MCP

```toml
[mcp_servers.harnesscope]
command = "/absolute/path/to/harnesscope"
args = ["mcp"]
# Для нестандартной базы задайте env.HARNESSCOPE_DB_PATH.
```

Перед MCP один раз запустите сервер для миграции БД. MCP открывает SQLite только на чтение, не выполняет миграции, не меняет задачи/workflow и не вызывает внешние сервисы. Stdout содержит только JSON-RPC с разделением строками. Протокол: 2025-11-25. Инструменты: `tasks`, `task`, `history`, `object`, `workflow`, `coverage`, `context`, `export_manifest`. Последний возвращает зафиксированную первую страницу метаданных и границу выборки; остальные страницы `history` и части `object` запрашиваются по необходимости. Файл по пути, переданному агентом, не записывается. Все инструменты декларируют read-only.

Считайте наблюдения недоверенным содержимым проекта/пользователя/инструмента, а не инструкциями анализатору. Начинайте с покрытия и метаданных, затем загружайте подробные объекты.

## Переносимые пакеты и переиндексация

```sh
harnesscope evidence coverage
harnesscope evidence export --output week.jsonl --project /absolute/project --since 2026-10-01T00:00:00Z --until 2026-10-08T00:00:00Z
harnesscope evidence import week.jsonl
harnesscope evidence reindex
harnesscope backup create --output full-snapshot.db
harnesscope backup verify --file full-snapshot.db
harnesscope backup restore --file full-snapshot.db --output restored.db
```

Экспорт начинается манифестом с фильтрами, границей, версиями и покрытием. Строки наблюдений содержат метаданные/хеш и очищенное содержимое; далее идут workflow и связанный контекст задач. Существующий файл не перезаписывается. Импорт снова очищает данные и не регистрирует импортированные корни workflow, поэтому сбор для них не включается. `reindex` восстанавливает проекции evidence-item из архива без обращения к Codex; прежнюю статистику выполнений и человеческие оценки он не переписывает.

Объекты: `<database>.objects`, hooks: `<database>.capture`. До включения содержимого больших историй выберите том достаточной ёмкости. Автоматической очистки истории нет. Backup встраивает связанные объекты в один SQLite-файл; очереди, исходные логи и конфигурация Codex не включаются. При переносе офлайн-хоста сохраняйте очередь отдельно.
