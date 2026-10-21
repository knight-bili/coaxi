# Embedded third-party components

## Public Suffix List data in OkHttp 4.12.0

- Parent artifact: `com.squareup.okhttp3:okhttp:4.12.0`
- Packaged component: `okhttp3/internal/publicsuffix/publicsuffixes.gz`
- Component: Public Suffix List
- Source:
  <https://github.com/square/okhttp/blob/parent-4.12.0/okhttp/src/test/resources/okhttp3/internal/publicsuffix/public_suffix_list.dat>
- Upstream project: <https://publicsuffix.org/>
- License: Mozilla Public License 2.0
- License text: `MPL-2.0.txt`
- Local modifications: none; the compiled data is provided by the unmodified
  OkHttp dependency.

OkHttp's embedded `NOTICE` identifies the same source and license. Both that
notice and the full MPL-2.0 text are included in the generated application
notice bundle.
