-- Simple T-SQL example
SELECT u.Name, u.Age
FROM Users u
WHERE u.Age > 18
ORDER BY u.Name;

INSERT INTO AuditLog (Action, Timestamp)
VALUES ('UserQuery', GETDATE());

DELETE FROM OldRecords WHERE CreatedAt < '2020-01-01';
