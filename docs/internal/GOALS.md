# Howllo Goals

## 1. Why Howllo

Howllo มีโอกาสสูงกว่าโปรเจกต์อย่าง Rooiam และ JotJum ในแง่การหาลูกค้ากลุ่ม SaaS ทั่วโลก เพราะปัญหาเรื่อง feedback, roadmap, duplicate requests, status visibility, และ customer communication เป็น pain point ที่แทบทุก SaaS เจอ

จุดแข็งของ Howllo คือ:

- ขายเป็น B2B SaaS ได้เร็วกว่า product ที่ใหญ่และกว้างกว่า
- ใช้ความรู้เดิมจาก Rooiam ได้ทันทีในเรื่อง tenant, permission, membership, audit
- product scope เล็กพอจะทำ MVP ได้เร็ว
- open-core และ self-hosting friendly
- มีทางขยายจาก feedback board ไปสู่ customer portal ได้ในอนาคต

Howllo ไม่ควรถูกมองเป็น forum clone

Howllo ควรถูกมองเป็น:

- feedback collection layer
- roadmap visibility layer
- decision communication layer
- future customer portal foundation


## 2. Core Product Goal

เป้าหมายหลักของ Howllo คือ:

> ช่วยให้ทีม SaaS รับ feedback, จัดระเบียบ feedback, ตัดสินใจ, สื่อสารสถานะ, และแปลง feedback ไปเป็น roadmap และ changelog ได้ในระบบเดียว

flow หลักของ product ต้องเป็น:

```text
Collect feedback
-> Organize feedback
-> Moderate feedback
-> Roadmap
-> Search
-> AI
```

ไม่ใช่:

```text
Collect feedback
-> AI everything
```


## 3. Product Thesis

Howllo จะชนะได้ ถ้ามันทำ 3 อย่างนี้ได้ดี:

1. ทำให้ลูกค้ารวบรวม feedback ได้ง่าย
2. ทำให้ทีม product/admin จัดการ feedback ได้เป็นระบบ
3. ทำให้ผู้ใช้เห็นความคืบหน้าได้ชัดเจนและเชื่อถือได้

คนไม่ได้ต้องการแค่ "ที่โพสต์ request"

คนต้องการ:

- รู้ว่ามีคนขอเรื่องเดียวกันไหม
- รู้ว่าทีมเห็นเรื่องนี้หรือยัง
- รู้ว่าตอนนี้อยู่สถานะอะไร
- รู้ว่าเมื่อไรเสร็จ
- รู้ว่าที่เสร็จแล้วออก release เมื่อไร


## 4. Strategic Principles

### 4.1 AI must come last

AI เป็น acceleration layer ไม่ใช่ product foundation

AI จะมีค่าก็ต่อเมื่อระบบพื้นฐานเหล่านี้ดีอยู่แล้ว:

- feedback collection
- duplicate handling
- moderation
- roadmap workflow
- search and discovery

ถ้าพื้นฐานพัง AI จะยิ่งทำให้ระบบมั่วขึ้น

### 4.2 Tenant-first from day one

Howllo ต้องออกแบบแบบ multi-tenant ตั้งแต่วันแรก

backend ต้องยึดหลัก:

- tenant isolation
- membership
- permission
- auditability

ห้ามแก้เรื่องนี้ทีหลัง เพราะ migration จะเจ็บมาก

### 4.3 Closed workflow, not user-defined chaos

status หลักต้องเป็น closed enum

ตัวอย่าง:

```rust
enum FeedbackStatus {
    UnderReview,
    Planned,
    InProgress,
    Done,
    Declined,
}
```

ไม่ควรให้ tenant สร้าง status เองในช่วงแรก เพราะจะพังทั้ง data quality, filtering, roadmap view, analytics, และ automation

### 4.4 Trust comes from history

roadmap จะไม่มีความหมาย ถ้าผู้ใช้ไม่รู้ว่าเกิดอะไรขึ้น

ทุก status transition ควรเก็บ:

- who
- when
- from
- to
- reason

นี่ไม่ใช่ feature เสริม แต่เป็น trust layer ของ product


## 5. What Howllo Must Be In V1

V1 ของ Howllo ต้องทำให้ loop นี้สมบูรณ์:

1. ผู้ใช้ส่ง feedback ได้
2. คนอื่น vote, comment, follow ได้
3. ทีมจัดหมวด, รวม duplicate, moderate ได้
4. ทีมเปลี่ยน status ได้แบบมีประวัติ
5. ผู้ใช้เห็น roadmap ได้ชัด
6. ผู้ใช้ติดตามความคืบหน้าได้
7. ทีมเอาของที่ done ไปต่อเป็น changelog หรือ release note ได้

V1 ที่ดีต้องให้ความรู้สึกว่า:

> "นี่คือที่เดียวที่ลูกค้าคุยกับทีม product แล้วเห็นผลลัพธ์กลับมา"


## 6. Must-have Capabilities

### 6.1 Feedback capture

- create post
- vote
- comment
- tag
- basic board separation

### 6.2 Follow / subscribe

นี่เป็น capability สำคัญและควรเข้ามาเร็ว

อย่างน้อยควรมี:

- follow post
- notify on status change
- notify on official response

เพราะ killer feature ของ feedback board ไม่ใช่แค่ "submit request"
แต่คือ

> "Tell me when this moves."

### 6.3 Moderation

- hide/unhide
- lock/unlock
- official team response
- duplicate merge or duplicate link
- admin notes

### 6.4 Roadmap visibility

- public roadmap view
- clear status progression
- public status history
- official updates

### 6.5 Trust and ownership

- export CSV
- export JSON
- auditability
- self-hostability


## 7. Product Model Decisions

### 7.1 Official response should be comment-based

official team response ไม่ควรแยกเป็น model ใหญ่เกินจำเป็นในช่วงแรก

แนวทางที่เหมาะกว่าในระยะแรกคือใช้ `comments` เดิม แล้วเพิ่ม field เช่น:

```rust
comment_type

User
Official
Moderator
```

ข้อดี:

- schema ง่ายกว่า
- timeline เดียวกัน
- UI render ต่างกันได้
- ไม่ต้องสร้าง abstraction เกินจำเป็น

### 7.2 Roadmap should be its own resource

ระยะต้น post อาจเป็นต้นทางของ roadmap ได้

แต่ API ไม่ควรผูก roadmap ไว้กับ:

```text
GET /posts?status=planned
```

ควรมี resource แบบ:

```text
GET /roadmap
```

เพราะในอนาคต roadmap อาจโตไปเป็น:

- milestone
- epic
- release
- group

### 7.3 Duplicate handling must come early

duplicate request เป็นปัญหาใหญ่ของ feedback products ทุกตัว

หลังเปิด public จะเจอหลายโพสต์ที่เป็นเรื่องเดียวกันทันที

ถ้าจัดการ duplicate ช้า:

- board จะรก
- vote จะกระจาย
- roadmap จะบิดเบือน
- user experience จะดูไม่ professional

ดังนั้น duplicate workflow ควรมาเร็วกว่า phase moderation ใหญ่


## 8. Product Expansion Path

Howllo ไม่ควรหยุดที่ feedback board

เส้นทางขยายที่มีเหตุผลคือ:

```text
Feedback
-> Roadmap
-> Changelog
-> Release Notes
-> Customer Portal
```

### 8.1 Changelog

ลูกค้าไม่ได้อยากรู้แค่ว่า "กำลังทำอะไร"

ลูกค้าอยากรู้ด้วยว่า:

> "อะไรเปลี่ยนไปแล้ว"

ดังนั้น changelog เป็น capability สำคัญ ไม่ใช่ของแถม

### 8.2 Release notes

สิ่งที่อยู่ในสถานะ `Done` ควรเชื่อมไปสู่ release note ได้

เช่น:

- Dark Mode
- Export CSV
- API Token

รวมเป็น release page ได้

### 8.3 Customer portal

ระยะยาว Howllo อาจกลายเป็นระบบที่รวม:

- feedback
- roadmap
- changelog
- knowledge surface

ซึ่งมีตลาดใหญ่กว่า feedback board เดี่ยวๆ


## 9. Recommended Phase Order

### Phase 0: Foundation

เป้าหมาย:

- stabilize backend
- close tenant/security gaps
- typed status
- status history
- validation
- tests

### Phase 1: Feedback Core

เป้าหมาย:

- post, vote, comment, tag
- follow/subscribe
- official comment types
- board management
- private/public visibility

### Phase 1.5: Duplicate Control

เป้าหมาย:

- mark duplicate
- link to canonical post
- consolidate user attention
- prepare future merge logic

### Phase 2: Moderation and Roadmap

เป้าหมาย:

- moderation notes
- transition guardrails
- admin workflow
- dedicated roadmap resource
- public roadmap view

### Phase 3: Production and Trust

เป้าหมาย:

- export CSV/JSON
- docs
- self-hosting
- audit logs
- abuse protection
- health/readiness

### Phase 4: Search and Discovery

เป้าหมาย:

- search
- ranking
- duplicate suggestions without AI dependency

### Phase 5: AI Assist

เป้าหมาย:

- duplicate suggestions
- clustering
- summarization
- moderation assist


## 10. What Makes Howllo Strong

Howllo จะมีโอกาสสูง ถ้ารักษาสมดุลนี้ได้:

- เล็กพอที่จะ shipping ได้เร็ว
- ลึกพอที่จะขายเป็น product จริง
- simple enough for self-hosting
- structured enough for SaaS teams
- trustworthy enough for product communication


## 11. Success Criteria

Howllo ถือว่ามาถูกทางเมื่อ:

- ทีม SaaS ใช้มันเป็น feedback inbox จริง
- ผู้ใช้กลับมาดู roadmap ซ้ำ
- duplicate ลดลงเพราะ search และ moderation ดีขึ้น
- ลูกค้ารู้สึกว่าข้อมูลเป็นของตัวเองผ่าน export/self-hosting
- `Done` items ต่อไปเป็น changelog หรือ release notes ได้
- ระบบขยายจาก feedback board ไปเป็น customer communication product ได้


## 12. Priority Order Among Current Products

ถ้าต้องเรียงลำดับเชิงธุรกิจ:

1. Rooiam
2. Howllo
3. JotJum

เหตุผล:

- Rooiam เป็นฐาน identity และ infra product ที่ใช้ต่อยอดได้กว้าง
- Howllo ขาย B2B ได้เร็วกว่าและใช้ competency เดิมได้เยอะ
- JotJum ใหญ่กว่าและมี execution risk สูงกว่าในช่วงนี้


## 13. Short Version

Howllo ควรถูกสร้างเป็น:

> multi-tenant feedback and roadmap product for SaaS teams, with strong trust, clear workflow, followability, duplicate control, roadmap visibility, and future expansion into changelog and customer portal

ถ้าทำตามนี้ได้ Howllo จะไม่ใช่แค่ "board ให้ลูกค้าโพสต์"

มันจะกลายเป็น:

> the product communication system between SaaS teams and their users
