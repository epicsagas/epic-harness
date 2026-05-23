---
name: php-care
description: "Preset: PHP files need lint and static analysis after edits."
---

# PHP file care (preset)

## Process
1. Run `php -l` to check syntax after editing `.php` files
2. Run `phpstan analyse` or `psalm` for static analysis
3. Run `phpcs` for coding standard checks

## Red Flags
- Skipping syntax check after editing PHP files
- Ignoring static analysis errors from phpstan/psalm
- Not running phpcs on changed files