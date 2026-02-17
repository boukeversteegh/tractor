-- Simple T-SQL example
SELECT u.Name, u.Age
FROM Users u
WHERE u.Age > 18
ORDER BY u.Name;

INSERT INTO AuditLog (Action, Timestamp)
VALUES ('UserQuery', GETDATE());

DELETE FROM OldRecords WHERE CreatedAt < '2020-01-01';

-- Bracket identifiers, schema-qualified names, variables
SELECT [dbo].[Users].[Name] AS UserName, @StartDate
FROM [dbo].[Users]
WHERE [Age] BETWEEN 18 AND 65;

-- UPDATE with variables and schema-qualified table
UPDATE dbo.Config SET Value = @NewValue WHERE [Key] = @KeyName;
