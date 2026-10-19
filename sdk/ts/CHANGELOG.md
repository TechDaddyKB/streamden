# Changelog

## Unreleased

- Add the private `@streamden/sdk` workspace package with strict typechecking and generated type exports.
- Add known/unknown node and fragment guards, including narrowing for sub-actions.
- Check protocol fixtures against TypeScript types and test schema-derived kind classification.
- Fix unknown-kind classifiers erasing the known TypeScript branch; narrow with negated known guards.
- Compile runtime guard tests independently so filtered runs and fixture failures do not prevent imports.
- Cover the r6 `for_each` kind and new protocol fixtures from DEN-48.
