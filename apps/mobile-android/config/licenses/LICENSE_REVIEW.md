# Artifact license review decisions

## Jakarta Dependency Injection API 2.0.1

- Artifact: `jakarta.inject:jakarta.inject-api:2.0.1`
- Selected license: Apache License 2.0
- Immediate Maven POM:
  <https://repo1.maven.org/maven2/jakarta/inject/jakarta.inject-api/2.0.1/jakarta.inject-api-2.0.1.pom>
- Tagged upstream notice:
  <https://github.com/eclipse-ee4j/injection-api/blob/2.0.1/NOTICE.md>
- Tagged upstream license:
  <https://github.com/eclipse-ee4j/injection-api/blob/2.0.1/LICENSE.txt>
- Packaged evidence: the JAR's `META-INF/LICENSE.txt` and `META-INF/NOTICE.md`
  are included in the generated notice bundle.

The artifact's immediate POM declares only Apache-2.0, its tagged notice says
the project material is made available under Apache-2.0 and carries
`SPDX-License-Identifier: Apache-2.0`, and the distributed JAR embeds the
Apache-2.0 text. Some Jakarta parent POMs advertise EPL-2.0 and GPL-2.0 with the
Classpath Exception as options for other project-family material. Those parent
licenses are not declarations for this resolved artifact.

The license report therefore sets `unionParentPomLicenses = false` and records
the immediate artifact declaration. This prevents unrelated parent metadata
from being presented as an obligation of the packaged JAR.
