# Руководство по Homebrew для Harnesscope

Harnesscope распространяется через официальный тап [`mr-lexus/homebrew-tap`](https://github.com/mr-lexus/homebrew-tap).

---

## 1. Быстрая установка

```bash
# Подключение тапа и установка harnesscope
brew install mr-lexus/tap/harnesscope
```

Проверка установки:
```bash
harnesscope --version
harnesscope doctor
```

---

## 2. Управление фоновым сервисом телеметрии

Homebrew позволяет запускать Harnesscope как фоновый системный демон (`launchd` на macOS, `systemd` на Linux):

```bash
# Автозапуск фонового сервера при входе в систему
brew services start mr-lexus/tap/harnesscope

# Проверка статуса сервиса
brew services list

# Остановка фонового сервера
brew services stop mr-lexus/tap/harnesscope

# Перезапуск сервиса
brew services restart mr-lexus/tap/harnesscope
```

При работе сервиса Homebrew:
- **Адрес Web UI и API**: `http://127.0.0.1:4242`
- **Логи**: `$(brew --prefix)/var/log/harnesscope.log`
- **Ошибки**: `$(brew --prefix)/var/log/harnesscope.error.log`

---

## 3. Обновление через Homebrew

```bash
brew update
brew upgrade mr-lexus/tap/harnesscope
brew services restart mr-lexus/tap/harnesscope
```

---

## 4. Запуск без системного демона

Если вы не хотите использовать `brew services`, Harnesscope имеет собственные кроссплатформенные команды:

```bash
# Открыть Web UI (при необходимости автоматически поднимает фоновый процесс)
harnesscope ui

# Управление демоном через CLI
harnesscope server start
harnesscope server status
harnesscope server stop
```
