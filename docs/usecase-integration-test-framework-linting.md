# Use Case: Linting an Integration Test Framework

This document describes a real-world use case for tractor: enforcing architectural design rules in a custom integration testing framework for a C# application.

## Context

We have a custom integration testing framework with specific design patterns:

- **Models**: Simple data classes representing domain entities (Customer, Order, Product, etc.)
- **Builders**: Classes that create domain objects via API calls using a fluent interface
- **Readers**: Classes that read data from APIs and map to models
- **Expectations**: Assertion helpers that verify expected vs actual state

The framework has strict design rules to ensure consistency, maintainability, and parallel test execution. We want to use tractor to automatically detect violations of these rules.

## Design Rules and Tractor Queries

---

### Rule M1: Model IDs Must Be Nullable

**Why**: Models are created before API calls. The ID is only known after persistence, so it must be nullable (`Guid?`) to represent "not yet persisted".

**Correct**:
```csharp
public class CustomerModel
{
    public Guid? Id { get; set; }  // Nullable - correct
    public string Email { get; set; }
}
```

**Violation**:
```csharp
public class CustomerModel
{
    public Guid Id { get; set; }  // Non-nullable - VIOLATION
    public string Email { get; set; }
}
```

**Tractor Query** (works today):
```bash
tractor "Models/**/*.cs" \
  -x "//property[type[not(nullable) and .='Guid']]" \
  --expect none \
  --error "Model has non-nullable Guid - should be Guid?"
```

**XML Structure**:
```xml
<!-- Nullable Guid? -->
<property>
  <type kind="nullable_type">
    Guid
    <nullable/>
  </type>
  <name>Id</name>
</property>

<!-- Non-nullable Guid (violation) -->
<property>
  <type>Guid</type>
  <name>Id</name>
</property>
```

---

### Rule M7: Collections Must Be Initialized

**Why**: Uninitialized collections cause NullReferenceExceptions. All collection properties should have default empty values.

**Correct**:
```csharp
public class OrderModel
{
    public List<OrderItemModel> Items { get; set; } = [];
    public List<Guid> ProductIds { get; set; } = new List<Guid>();
}
```

**Violation**:
```csharp
public class OrderModel
{
    public List<OrderItemModel> Items { get; set; }  // No initializer - VIOLATION
}
```

**Tractor Query** (works today):
```bash
tractor "Models/**/*.cs" \
  -x "//property[type[contains(.,'List')] and not(default)]" \
  --expect none \
  --error "Collection property must have default initializer"
```

---

### Rule B1: Builders Must Make Exactly One API Call

**Why**: Each builder is responsible for creating exactly one entity. Multiple API calls indicate the builder is doing too much or has incorrect separation of concerns.

**Correct**:
```csharp
public class OrderBuilder : Builder
{
    protected override async Task BuildMainEntity()
    {
        var dto = MapToDto();
        var result = await api.CreateOrderAsync(dto);  // Single API call
        Model.Id = result.Id;
    }
}
```

**Violation**:
```csharp
public class OrderBuilder : Builder
{
    protected override async Task BuildMainEntity()
    {
        var result = await api.CreateOrderAsync(dto);  // First call
        Model.Id = result.Id;

        var details = await api.GetOrderAsync(result.Id);  // Second call - VIOLATION
        Model.Status = details.Status;
    }
}
```

**Tractor Query** (partially works - may have false positives):
```bash
# Count await calls per file
tractor "Builders/**/*.cs" \
  -x "//method[name[.='BuildMainEntity']]//await" \
  -o count
```

**Limitation**: This counts all `await` calls, but some builders legitimately have multiple awaits in different branches of a switch/if statement (only one executes). We need **per-method counting with a maximum**.

**Desired Feature**:
```bash
# Hypothetical: fail if any BuildMainEntity method has more than 1 await
tractor "Builders/**/*.cs" \
  -x "//method[name[.='BuildMainEntity']]" \
  --expect-max-children "//await" 1 \
  --error "BuildMainEntity should have at most 1 API call"
```

---

### Rule B6: Builders Must Not Accept Guid Parameters

**Why**: Builders should receive Model objects, not raw IDs. This ensures type safety and allows the framework to automatically resolve IDs when needed.

**Correct**:
```csharp
public class OrderItemBuilder : Builder
{
    public OrderItemBuilder ForOrder(OrderModel order)
    {
        _order = order;
        return this;
    }
}
```

**Violation**:
```csharp
public class OrderItemBuilder : Builder
{
    public OrderItemBuilder ForOrder(Guid orderId)  // VIOLATION - accepts Guid
    {
        _orderId = orderId;
        return this;
    }
}
```

**Tractor Query** (works today):
```bash
tractor "Builders/**/*.cs" \
  -x "//method[public and parameter[type[.='Guid']]]" \
  --expect none \
  --error "Builder methods should accept Models, not Guids"
```

---

### Rule B7: No Builder-to-Builder Dependencies

**Why**: Builders should not store references to other builders. Dependencies are passed via Model objects. Storing builder references creates coupling and ordering issues.

**Correct**:
```csharp
public class ShipmentBuilder : Builder
{
    private OrderModel _order;  // Stores MODEL reference - correct

    public ShipmentBuilder ForOrder(OrderModel order)
    {
        _order = order;
        return this;
    }
}
```

**Violation**:
```csharp
public class ShipmentBuilder : Builder
{
    private OrderBuilder _orderBuilder;  // Stores BUILDER reference - VIOLATION

    public ShipmentBuilder ForOrder(OrderBuilder builder)
    {
        _orderBuilder = builder;
        return this;
    }
}
```

**Tractor Query** (works today):
```bash
tractor "Builders/**/*.cs" \
  -x "//field[type[contains(.,'Builder')]]" \
  --expect none \
  --error "Builders should not store references to other builders"
```

---

### Rule B11: Fluent Methods Must Return Builder Type

**Why**: All configuration methods should return `this` to enable method chaining.

**Correct**:
```csharp
public class OrderBuilder : Builder
{
    public OrderBuilder WithCustomer(CustomerModel customer)
    {
        Model.Customer = customer;
        return this;  // Returns builder - correct
    }
}
```

**Violation**:
```csharp
public class OrderBuilder : Builder
{
    public void WithCustomer(CustomerModel customer)  // Returns void - VIOLATION
    {
        Model.Customer = customer;
    }
}
```

**Tractor Query** (works today):
```bash
tractor "Builders/**/*.cs" \
  -x "//class[contains(name,'Builder')]//method[public and returns[.='void'] and not(name[.='BuildMainEntity'])]" \
  --expect none \
  --error "Fluent methods should return the builder type, not void"
```

---

### Rule B12: Builders Must Implement IModelBuilder

**Why**: All builders should implement the `IModelBuilder<TModel, TBuilder>` interface for consistency and to enable generic operations.

**Correct**:
```csharp
public class OrderBuilder : Builder, IModelBuilder<OrderModel, OrderBuilder>
{
    public OrderModel Model { get; set; } = new();
}
```

**Violation**:
```csharp
public class OrderBuilder : Builder  // Missing IModelBuilder - VIOLATION
{
    public OrderModel Model { get; set; } = new();
}
```

**Tractor Query** (works today):
```bash
tractor "Builders/**/*.cs" \
  -x "//class[ends-with(name,'Builder') and not(base_list[contains(.,'IModelBuilder')])]" \
  --expect none \
  --error "Builder must implement IModelBuilder<TModel, TBuilder>"
```

---

### Rule R2: Readers Must Have Query and Model Properties

**Why**: Readers follow a Query/Response pattern. The Query holds input parameters, the Model holds the response data.

**Correct**:
```csharp
public class CustomerReader : Builder, IReader<CustomerModel, CustomerReader.QueryModel, CustomerReader>
{
    public CustomerModel Model { get; set; } = new();  // Response
    public QueryModel Query { get; private set; } = new();  // Input

    public class QueryModel
    {
        public CustomerModel? Customer { get; set; }
    }
}
```

**Violation**:
```csharp
public class CustomerReader : Builder  // Missing Query property - VIOLATION
{
    public CustomerModel Model { get; set; } = new();
    private Guid _customerId;  // Using raw field instead of QueryModel
}
```

**Tractor Query** (works today):
```bash
tractor "Readers/**/*.cs" \
  -x "//class[ends-with(name,'Reader') and not(.//property[name[.='Query']])]" \
  --expect none \
  --error "Reader must have a Query property"
```

---

### Rule T1: Tests Must Not Use Static/Shared Data

**Why**: Tests run in parallel. Shared static data causes race conditions and flaky tests.

**Correct**:
```csharp
[Test]
public async Task CreateCustomer_SetsEmail()
{
    // Each test creates its own data
    await Builder
        .Customer(c => c.WithEmail("test@example.com"))
        .BuildAsync();
}
```

**Violation**:
```csharp
public class CustomerTests
{
    private static CustomerModel _sharedCustomer;  // VIOLATION - static shared data

    [SetUp]
    public void Setup()
    {
        _sharedCustomer = CreateCustomer();
    }
}
```

**Tractor Query** (partially works):
```bash
tractor "Tests/**/*.cs" \
  -x "//field[static and type[contains(.,'Model')]]" \
  --expect none \
  --error "Tests should not use static Model fields - causes parallel test issues"
```

---

## Feature Requests for Tractor

### 1. Per-Context Counting

**Need**: Enforce "at most N matches within a specific scope"

**Use Case**: Rule B1 - "BuildMainEntity should have at most 1 await call"

**Current Limitation**: Can only count globally, not per-method

**Proposed Syntax**:
```bash
tractor "**/*.cs" \
  -x "//method[name[.='BuildMainEntity']]" \
  --expect-max-children "//await" 1
```

### 2. Conditional Exclusion

**Need**: Exclude matches inside certain constructs (switch, if, try/catch)

**Use Case**: Multiple awaits are OK if they're in different switch branches

**Proposed Syntax**:
```bash
tractor "**/*.cs" \
  -x "//method[name[.='BuildMainEntity']]//await[not(ancestor::switch_section)]" \
  --expect-max 1
```

### 3. Named Captures in Error Messages

**Need**: Include matched element names/values in error messages

**Current**: `{name}` placeholder doesn't resolve

**Proposed**:
```bash
tractor "**/*.cs" \
  -x "//property[type[.='Guid']]" \
  --error "Property '{./name}' has non-nullable Guid in {file}:{line}"
```

### 4. Cross-Reference Checking

**Need**: Verify that a type referenced in one file exists in another

**Use Case**: Ensure all `IModelBuilder<TModel>` references point to existing Model classes

**Proposed Syntax**:
```bash
tractor "**/*.cs" \
  -x "//base_list//generic_name[ref[.='IModelBuilder']]//type_argument" \
  --exists-in "Models/**/*.cs" "//class[name[.='{value}']]"
```

---

## Summary

Tractor is already useful for detecting many architectural violations in our codebase. The most impactful additions would be:

1. **Per-scope counting** - to properly enforce "exactly one API call" rules
2. **Better error message placeholders** - for actionable violation reports
3. **Ancestor exclusion** - to handle legitimate conditional branches

These features would allow us to run tractor in CI to automatically catch design rule violations before code review.
