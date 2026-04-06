---
title: Complete Transformation Example
priority: 2
---

End-to-end example showing TreeSitter input and semantic tree output.

## Input C# Code

```csharp
public static class QueryHelpers
{
    public static IQueryable<T> Where<T>(this IQueryable<T> source)
    {
        return source;
    }
}
```

## TreeSitter Raw Output

```xml
<class_declaration line="1" column="1" end_line="7" end_column="2">
  <modifier>public</modifier>
  <modifier>static</modifier>
  <identifier>QueryHelpers</identifier>
  <declaration_list>
    <method_declaration line="3" column="5" end_line="6" end_column="6">
      <modifier>public</modifier>
      <modifier>static</modifier>
      <generic_name>
        <identifier>IQueryable</identifier>
        <type_argument_list>
          <identifier>T</identifier>
        </type_argument_list>
      </generic_name>
      <identifier>Where</identifier>
      <type_parameter_list>
        <type_parameter>
          <identifier>T</identifier>
        </type_parameter>
      </type_parameter_list>
      <parameter_list>
        <parameter>
          <modifier>this</modifier>
          <generic_name>
            <identifier>IQueryable</identifier>
            <type_argument_list>
              <identifier>T</identifier>
            </type_argument_list>
          </generic_name>
          <identifier>source</identifier>
        </parameter>
      </parameter_list>
      <block>
        <return_statement>
          <identifier>source</identifier>
        </return_statement>
      </block>
    </method_declaration>
  </declaration_list>
</class_declaration>
```

## Semantic Tree Output

```xml
<class line="1" column="1" end_line="7" end_column="2">
  <public/><static/>
  <name>QueryHelpers</name>
  <method line="3" column="5" end_line="6" end_column="6">
    <public/><static/>
    <name>Where</name>
    <type>
      <generic><name>IQueryable</name><type><name>T</name></type></generic>
    </type>
    <typeparams>
      <typeparam><name>T</name></typeparam>
    </typeparams>
    <params>
      <param>
        <this/>
        <name>source</name>
        <type>
          <generic><name>IQueryable</name><type><name>T</name></type></generic>
        </type>
      </param>
    </params>
    <block>
      <return><name>source</name></return>
    </block>
  </method>
</class>
```

## XPath Queries Now Possible

| Question | XPath |
|----------|-------|
| Find static classes | `//class[static]` |
| Find extension methods | `//method[params/param[this]]` |
| Find Where method | `//method[name='Where']` |
| Find generic methods | `//method[typeparams]` |
| Methods returning IQueryable | `//method[type/generic/name='IQueryable']` |
