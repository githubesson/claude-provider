# claude-provider

manage your claude code configs and switch between them easily.

## install

```
cargo install --path .
```

## usage

### setup a new provider

```
claude-provider setup
```

follow the prompts to enter provider name, api base url, api key, and models.

### list configured providers

```
claude-provider list
```

### remove a provider

```
claude-provider remove
```

### run claude with a specific provider

```
claude-provider use <provider-name> [args]
```

### interactive menu

```
claude-provider interactive
```

## how it works

each provider is stored as a json file in `~/.claude/providers/`. when you run `claude-provider use`, it temporarily modifies `~/.claude/settings.json` with the provider's configuration, runs claude, then restores the original settings.

## zsh integration

when you set up a provider, it automatically creates a zsh function in `~/.claude/provider-functions.zsh` and adds a source line to your `~/.zshrc`. this lets you run:

```
<provider-name> [args]
```

as a shortcut for `claude-provider use <provider-name> [args]`.
