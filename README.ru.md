# Harnesscope

> **Локальная кроссплатформенная утилита телеметрии для AI coding agents.**

Harnesscope прозрачно оборачивает CLI и GUI инструменты AI-агентов (Codex, Copilot, OpenCode, Cursor, VS Code, Aider), детерминированно собирает телеметрию выполнения без обращения к LLM, сохраняет данные локально в SQLite и отображает их во встроенном Web UI.

---

## Установка

### Homebrew (macOS и Linux)
```bash
brew install mr-lexus/tap/harnesscope

# Опционально: автозапуск сервера как системного сервиса в фоне
brew services start mr-lexus/tap/harnesscope
```

### Скрипт прямой установки
```bash
# macOS и Linux:
curl -fsSL https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.sh | bash

# Windows (PowerShell):
irm https://raw.githubusercontent.com/mr-lexus/harnesscope/main/install.ps1 | iex
```

### Сборка из исходников
```bash
cd web && npm install && npm run build && cd ..
cargo build --release
```

---

## Быстрый старт

### 1. Запуск рабочих сред агентов
Запускайте агенты через Harnesscope. Стандартные потоки ввода/вывода (stdin, stdout, stderr), интерактивный терминал TTY и коды завершения передаются прозрачно:

```bash
# Интерактивные CLI / TUI среды
harnesscope codex
harnesscope copilot
harnesscope opencode

# Любые редакторы, студии и CLI-агенты
harnesscope run cursor .
harnesscope run code .
harnesscope run --runner aider aider
```

*Примечание: При запуске любого агента Harnesscope автоматически поднимает локальный сервер телеметрии в фоне, если он ещё не был запущен.*

### 2. Просмотр телеметрии и Web UI
```bash
# Открыть Web UI в браузере по умолчанию (http://127.0.0.1:4242)
harnesscope ui

# Управление сервером вручную
harnesscope server start
harnesscope server status
harnesscope server stop

# Проверка окружения и путей к агентам
harnesscope doctor
```

### 3. Загрузка демо-данных (опционально)
```bash
harnesscope demo seed
```

---

## Ключевые принципы

- **Никаких вызовов LLM**: Harnesscope никогда не запрашивает нейросеть для определения моделей, сессий или инструментов. Все факты извлекаются строго детерминированно: из аргументов процесса, системного окружения, Git, конфигураций и логов. Если факт неизвестен — сохраняется `UNKNOWN`.
- **Корректная модель идентичности**: Чётко разделяет **Runtime** (процесс ОС), **Session** (логический диалог) и **Execution** (отдельная попытка/turn). Если сессия возобновляется после перезапуска процесса (`process stopped != session stopped`), Harnesscope сохраняет непрерывность сессии.
- **Git-контекст и отслеживание конфликтов**: Фиксирует снимки состояния Git до и после выполнения (HEAD SHA, ветка, dirty-статус, изменённые файлы, diff stat). Если два агента одновременно работают в одном worktree, атрибуция изменений помечается как `AMBIGUOUS`.
- **Локальность и приватность**: Все данные хранятся локально в SQLite (режим WAL). API-ключи, токены, пароли и заголовки авторизации автоматически маскируются перед записью.

---

## Документация

- [Архитектура системы](docs/ARCHITECTURE.md)
- [Руководство по Homebrew](docs/HOMEBREW.ru.md)
- [Кроссплатформенность и запуск GUI](docs/CROSS_PLATFORM.ru.md)
- [Руководство разработчика (Русский)](docs/DEVELOPMENT.ru.md)
- [README (English)](README.md)
- [Homebrew Guide (English)](docs/HOMEBREW.md)
- [Cross-Platform & GUI (English)](docs/CROSS_PLATFORM.md)
- [Developer Guide (English)](docs/DEVELOPMENT.md)

